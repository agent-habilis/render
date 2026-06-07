use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Rect, Size};
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;

use super::model::{Deck, parse_deck};
use super::nav::{Action, action_for};
use super::segment::Segment;
use super::view::{FOOTER_ROWS, content_width, draw_error};
use super::components::tty::ProcessManager;
use super::components::{Cached, FrameCtx, ImageStore};

/// Input-poll timeout: how often an idle slide wakes to animate and blink.
const TICK: Duration = Duration::from_millis(100);
/// Half-period of the running-status blink.
const BLINK_MS: u128 = 500;

/// Render a deck full-screen and navigate it until the user quits. `<img>` paths
/// resolve against the deck file's directory; `watch` live-reloads on file change.
pub(crate) fn run(input: &str, path: &Path, watch: bool) -> Result<()> {
    let deck = parse_deck(input)?;
    if deck.slides.is_empty() {
        bail!("no slides found: separate slides with `---` lines");
    }

    let mut terminal = ratatui::init();
    // Detect the graphics protocol *after* entering the alternate screen, as
    // `from_query_stdio` requires (it reads/writes stdio).
    let picker = crate::img::detect_picker();

    // The `App` (and so every running process) drops at the end of this
    // statement — killing its children — before the terminal is restored.
    let result = App::new(deck, picker, path.to_path_buf(), watch).run(&mut terminal);
    ratatui::restore();
    // Clear any OSC 9;4 indicator unconditionally — a reload can have toggled
    // `progress` on, and clearing when it was never set is a harmless no-op.
    clear_terminal_progress();
    result
}

/// OSC 9;4 progress: state 1 sets the bar to `pct` (0–100). Written straight to
/// stdout (the same fd ratatui draws to) between repaints.
fn set_terminal_progress(pct: u8) {
    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "\x1b]9;4;1;{pct}\x1b\\").and_then(|()| stdout.flush());
}

/// OSC 9;4 state 0 removes the indicator (no progress field).
fn clear_terminal_progress() {
    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "\x1b]9;4;0\x1b\\").and_then(|()| stdout.flush());
}

/// The deck-position percentage for slide `current` (0-based) of `total`. A
/// single-slide deck is always complete.
fn progress_pct(current: usize, total: usize) -> u8 {
    if total <= 1 {
        return 100;
    }
    let pct = (current + 1) * 100 / total;
    u8::try_from(pct.min(100)).unwrap_or(100)
}

#[derive(Debug)]
pub(super) struct App {
    pub(super) deck: Deck,
    pub(super) manager: ProcessManager,
    pub(super) images: ImageStore,
    pub(super) current: usize,
    pub(super) start: Instant,
    /// Reset on every navigation, so a slide's `<animation>`s restart when it opens.
    pub(super) slide_started: Instant,
    pub(super) static_render: StaticRender,
    /// The deck file, re-read on change when `watch` is set.
    pub(super) path: PathBuf,
    pub(super) watch: bool,
    /// File mtime at the last (re)load; `None` until first stat.
    pub(super) last_modified: Option<SystemTime>,
    /// Set while the watched file fails to parse, so the loop shows the error
    /// instead of a slide and leaves the last-good processes running.
    pub(super) reload_error: Option<String>,
}

/// Per-slide renders keyed by `(slide, content_w)`, reused across repaints so a
/// live process or animation does not re-run termimad/figurehead/syntect every
/// frame.
#[derive(Debug, Default)]
pub(super) struct StaticRender {
    pub(super) key: Option<(usize, usize)>,
    pub(super) items: Vec<Cached>,
}

impl App {
    fn new(deck: Deck, picker: Picker, path: PathBuf, watch: bool) -> Self {
        let base_dir = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let last_modified = std::fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        Self {
            deck,
            manager: ProcessManager::new(),
            images: ImageStore::new(picker, base_dir),
            current: 0,
            start: Instant::now(),
            slide_started: Instant::now(),
            static_render: StaticRender::default(),
            path,
            watch,
            last_modified,
            reload_error: None,
        }
    }

    /// Decode every `<img>` and spawn every `<tty prerender>` across all slides,
    /// up front, so a slow decode/spawn is paid once at load (or reload) rather
    /// than freezing navigation on a slide's first view.
    fn preload(&mut self, size: Size) {
        let pre_rows = size.height.saturating_sub(FOOTER_ROWS);
        let pre_cols = u16::try_from(content_width(size.width)).unwrap_or(u16::MAX);
        for slide in 0..self.deck.slides.len() {
            let segments = self.deck.slides[slide].segments();
            for (index, segment) in segments.iter().enumerate() {
                if let Segment::Tty(component) = segment
                    && component.prerender
                {
                    self.manager
                        .prerender(slide, index, component, pre_rows, pre_cols);
                }
            }
            self.images.ensure(slide, &segments);
        }
    }

    /// Re-read the watched file when its mtime changed; returns whether anything
    /// changed (so the caller forces a repaint). A failed stat (e.g. a mid-save
    /// atomic rename) is skipped and retried on the next tick.
    fn poll_reload(&mut self, size: Size) -> bool {
        let Ok(modified) = std::fs::metadata(&self.path).and_then(|meta| meta.modified()) else {
            return false;
        };
        if Some(modified) == self.last_modified {
            return false;
        }
        match std::fs::read_to_string(&self.path) {
            // Commit the mtime only once the read succeeds: a transient read
            // failure (a mid-save replace) then retries on the next tick rather
            // than sticking on the error until the file is touched again. A read
            // that *parses* badly does commit, so we don't re-parse the same bad
            // bytes every tick.
            Ok(input) => {
                self.last_modified = Some(modified);
                self.apply_reload(&input, size);
            }
            Err(err) => {
                self.reload_error = Some(format!("cannot read `{}`: {err}", self.path.display()));
            }
        }
        true
    }

    /// Replace the deck from freshly-read `input`. On success the current page is
    /// clamped to the closest existing slide and processes are left running; on a
    /// parse failure / empty deck the last-good deck is kept and an error is shown.
    fn apply_reload(&mut self, input: &str, size: Size) {
        match parse_deck(input) {
            Ok(deck) if !deck.slides.is_empty() => {
                self.deck = deck;
                self.current = self.current.min(self.deck.slides.len() - 1);
                // Running processes are intentionally kept alive — that is the
                // point of `--watch`. The forced reconcile below prunes ephemeral
                // procs off the current slide; here we drop prerendered procs for
                // slides the reload removed, which reconcile never touches (a
                // `<tty>` whose command merely changed in place keeps running).
                self.manager
                    .retain_prerendered_below(self.deck.slides.len());
                // Image keys are `(slide, index)`, so edits that shift slides
                // would otherwise serve a stale image — drop and re-decode.
                self.images.clear();
                // The render cache is keyed by `(slide, width)`; a same-page reload
                // wouldn't change that key, so invalidate it to re-render the new
                // content.
                self.static_render = StaticRender::default();
                self.preload(size);
                self.reload_error = None;
            }
            Ok(_) => {
                self.reload_error =
                    Some("no slides found: separate slides with `---` lines".to_owned());
            }
            Err(err) => self.reload_error = Some(format!("{err:#}")),
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Decode images / spawn prerendered processes up front so first view of a
        // slide doesn't freeze.
        self.preload(terminal.size().unwrap_or(Size::new(80, 24)));

        // `usize::MAX` forces the first iteration to reconcile (spawn slide 0).
        let mut reconciled_for = usize::MAX;
        // Last (slide, cols, rows) painted; gates repaints so an idle slide
        // doesn't re-render every tick.
        let mut drawn: Option<(usize, u16, u16)> = None;

        loop {
            let Size {
                width: cols,
                height: rows,
            } = terminal.size().unwrap_or(Size::new(80, 24));

            // Live reload: re-read the file when it changed. A successful reload
            // forces a reconcile of the (possibly clamped) current slide; a failed
            // one drops into the error screen, leaving processes running.
            if self.watch && self.poll_reload(Size::new(cols, rows)) {
                drawn = None;
                if self.reload_error.is_none() {
                    reconciled_for = usize::MAX;
                }
            }

            // Recomputed each tick: a reload can change the slide count.
            let total = self.deck.slides.len();
            let segments = self.deck.slides[self.current].segments();

            if self.reload_error.is_none() && reconciled_for != self.current {
                self.manager.reconcile(self.current, &segments);
                reconciled_for = self.current;
                self.slide_started = Instant::now();
                // Force a repaint of the whole new slide (not `terminal.clear()`,
                // which provokes a cursor-position query that some terminals never
                // answer): the prior slide's `<img>` cells are part of the diff, so
                // re-drawing them as the new content overwrites the old graphic.
                drawn = None;
                if self.deck.progress {
                    set_terminal_progress(progress_pct(self.current, total));
                }
            }

            let content_w = content_width(cols);
            let content_rows = rows.saturating_sub(FOOTER_ROWS);
            let content_cols = u16::try_from(content_w).unwrap_or(u16::MAX);
            if self.reload_error.is_none() {
                self.manager
                    .ensure(self.current, &segments, content_rows, content_cols);
                self.refresh_static_render(&segments, content_w);
            }

            let content_area = Rect::new(0, 0, cols, content_rows);
            let active = self.reload_error.is_none() && self.any_active(&segments, content_area);
            if drawn != Some((self.current, cols, rows)) || active {
                let blink_on = (self.start.elapsed().as_millis() / BLINK_MS).is_multiple_of(2);
                terminal.draw(|frame| {
                    if let Some(message) = &self.reload_error {
                        draw_error(frame, message);
                    } else {
                        self.draw(frame, &segments, blink_on);
                    }
                })?;
                drawn = Some((self.current, cols, rows));
            }

            if event::poll(TICK)? {
                let ev = event::read()?;
                if let Event::Key(key) = ev {
                    if key.kind == KeyEventKind::Press {
                        let action = action_for(&key);
                        if matches!(action, Some(Action::Quit)) {
                            return Ok(());
                        }
                        // Navigation is meaningless while an error is showing.
                        if self.reload_error.is_none() {
                            match action {
                                Some(Action::Next) => {
                                    self.current = (self.current + 1).min(total - 1);
                                }
                                Some(Action::Prev) => {
                                    self.current = self.current.saturating_sub(1);
                                }
                                Some(Action::First) => self.current = 0,
                                Some(Action::Last) => self.current = total - 1,
                                Some(Action::Quit) | None => {}
                            }
                        }
                    }
                } else if matches!(ev, Event::Resize(..)) {
                    drawn = None;
                }
            }
        }
    }

    fn refresh_static_render(&mut self, segments: &[Segment], content_w: usize) {
        let key = (self.current, content_w);
        if self.static_render.key == Some(key) {
            return;
        }
        let items = segments
            .iter()
            .map(|segment| segment.as_component().render(content_w))
            .collect();
        self.static_render = StaticRender {
            key: Some(key),
            items,
        };
    }

    pub(super) fn frame_ctx<'frame>(
        &'frame self,
        cached: &'frame Cached,
        index: usize,
        content: Rect,
        blink_on: bool,
    ) -> FrameCtx<'frame> {
        FrameCtx {
            cached,
            manager: &self.manager,
            images: &self.images,
            elapsed: self.slide_started.elapsed(),
            current: self.current,
            index,
            content,
            content_w: content_width(content.width),
            blink_on,
        }
    }

    fn any_active(&self, segments: &[Segment], content: Rect) -> bool {
        segments.iter().enumerate().any(|(index, segment)| {
            self.static_render.items.get(index).is_some_and(|cached| {
                segment
                    .as_component()
                    .is_active(&self.frame_ctx(cached, index, content, false))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{App, Deck, parse_deck, progress_pct};
    use super::super::components::Cached;
    use ratatui::layout::{Rect, Size};
    use ratatui_image::picker::Picker;
    use std::path::PathBuf;

    fn app(deck: Deck) -> App {
        App::new(deck, Picker::halfblocks(), PathBuf::from("deck.ahr-deck"), false)
    }

    const SIZE: Size = Size {
        width: 80,
        height: 24,
    };

    #[test]
    fn reload_clamps_current_to_the_new_last_slide() {
        let mut app = app(parse_deck("# 1\n\n---\n\n# 2\n\n---\n\n# 3\n\n---\n\n# 4\n\n---\n\n# 5").unwrap());
        app.current = 4; // last of 5
        app.apply_reload("# 1\n\n---\n\n# 2\n\n---\n\n# 3", SIZE); // now 3 slides
        assert_eq!(app.deck.slides.len(), 3);
        assert_eq!(app.current, 2); // closest smaller: the new last slide
        assert!(app.reload_error.is_none());
    }

    #[test]
    fn reload_keeps_current_when_still_valid() {
        let mut app = app(parse_deck("# 1\n\n---\n\n# 2\n\n---\n\n# 3").unwrap());
        app.current = 1;
        app.apply_reload("# A\n\n---\n\n# B\n\n---\n\n# C\n\n---\n\n# D", SIZE);
        assert_eq!(app.current, 1);
        assert_eq!(app.deck.slides.len(), 4);
    }

    #[test]
    fn reload_failure_keeps_last_good_deck_and_sets_error() {
        let mut app = app(parse_deck("# 1\n\n---\n\n# 2").unwrap());
        app.current = 1;
        // Two `<deck>` tags is a hard parse error.
        app.apply_reload("<deck title=\"a\" />\n\n# x\n\n---\n\n<deck title=\"b\" />\n\n# y", SIZE);
        assert!(app.reload_error.is_some());
        assert_eq!(app.deck.slides.len(), 2); // unchanged
        assert_eq!(app.current, 1);
    }

    #[test]
    fn reload_to_empty_is_an_error() {
        let mut app = app(parse_deck("# 1").unwrap());
        app.apply_reload("   \n\n  ", SIZE);
        assert!(app.reload_error.is_some());
        assert_eq!(app.deck.slides.len(), 1); // last-good kept
    }

    #[test]
    fn progress_pct_reports_deck_position() {
        assert_eq!(progress_pct(0, 1), 100); // single slide is complete
        assert_eq!(progress_pct(0, 4), 25);
        assert_eq!(progress_pct(3, 4), 100); // last slide
        assert_eq!(progress_pct(0, 0), 100); // degenerate
    }

    #[test]
    fn static_render_caches_static_segments_not_processes() {
        let deck = parse_deck(
            "# H\n\n<mermaid>\nflowchart LR; A --> B\n</mermaid>\n\n<tty>\necho hi\n</tty>",
        )
        .unwrap();
        let segments = deck.slides[0].segments();
        let mut app = app(deck);

        app.refresh_static_render(&segments, 80);
        assert_eq!(app.static_render.key, Some((0, 80)));
        assert_eq!(app.static_render.items.len(), segments.len());
        let cached = app
            .static_render
            .items
            .iter()
            .filter(|item| !matches!(item, Cached::Live))
            .count();
        let live = app
            .static_render
            .items
            .iter()
            .filter(|item| matches!(item, Cached::Live))
            .count();
        assert_eq!(cached, 2);
        assert_eq!(live, 1);

        app.refresh_static_render(&segments, 80);
        assert_eq!(app.static_render.key, Some((0, 80)));

        app.refresh_static_render(&segments, 60);
        assert_eq!(app.static_render.key, Some((0, 60)));
    }

    #[test]
    fn code_segment_is_cached_statically() {
        let deck = parse_deck("<code lang=\"rust\">\nfn main() {}\n</code>").unwrap();
        let segments = deck.slides[0].segments();
        let mut app = app(deck);

        app.refresh_static_render(&segments, 80);
        assert_eq!(app.static_render.items.len(), 1);
        assert!(
            matches!(app.static_render.items[0], Cached::Static(..)),
            "code must be cached"
        );
    }

    #[test]
    fn animation_caches_frames_and_advances_on_the_clock() {
        let deck = parse_deck(
            "<animation seconds=\"0.2\">\n<frame>\nA\n</frame>\n<frame>\nB\n</frame>\n<frame>\nC\n</frame>\n</animation>",
        )
        .unwrap();
        let segments = deck.slides[0].segments();
        let mut app = app(deck);
        app.refresh_static_render(&segments, 80);

        match &app.static_render.items[0] {
            Cached::Frames(frames) => assert_eq!(frames.len(), 3),
            Cached::Live | Cached::Static(..) => panic!("animation must cache frames"),
        }

        // is_active flips false once a non-looping animation reaches its last frame.
        let area = Rect::new(0, 0, 80, 24);
        let now = std::time::Instant::now();
        app.slide_started = now;
        assert!(app.any_active(&segments, area));
        app.slide_started = now.checked_sub(std::time::Duration::from_secs(10)).unwrap();
        assert!(!app.any_active(&segments, area)); // settled at last frame
    }
}
