use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Rect, Size};
use ratatui::DefaultTerminal;

use super::model::{Deck, parse_deck};
use super::nav::{Action, action_for};
use super::segment::Segment;
use super::view::{FOOTER_ROWS, content_width};
use super::components::tty::ProcessManager;
use super::components::{Cached, FrameCtx};

/// Input-poll timeout: how often an idle slide wakes to animate and blink.
const TICK: Duration = Duration::from_millis(100);
/// Half-period of the running-status blink.
const BLINK_MS: u128 = 500;

/// Render a deck full-screen and navigate it until the user quits.
pub(crate) fn run(input: &str) -> Result<()> {
    let deck = parse_deck(input)?;
    if deck.slides.is_empty() {
        bail!("no slides found: separate slides with `---` lines");
    }

    let mut terminal = ratatui::init();
    // The `App` (and so every running process) drops at the end of this
    // statement — killing its children — before the terminal is restored.
    let result = App::new(deck).run(&mut terminal);
    ratatui::restore();
    result
}

#[derive(Debug)]
pub(super) struct App {
    pub(super) deck: Deck,
    pub(super) manager: ProcessManager,
    pub(super) current: usize,
    pub(super) start: Instant,
    /// Reset on every navigation, so a slide's `<animation>`s restart when it opens.
    pub(super) slide_started: Instant,
    pub(super) static_render: StaticRender,
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
    fn new(deck: Deck) -> Self {
        Self {
            deck,
            manager: ProcessManager::new(),
            current: 0,
            start: Instant::now(),
            slide_started: Instant::now(),
            static_render: StaticRender::default(),
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let total = self.deck.slides.len();

        // Spawn `prerender` processes up front (sized to an estimate) so a
        // slow-starting program is already warmed up when its slide is reached.
        {
            let size = terminal.size().unwrap_or(Size::new(80, 24));
            let pre_rows = size.height.saturating_sub(FOOTER_ROWS);
            let pre_cols = u16::try_from(content_width(size.width)).unwrap_or(u16::MAX);
            for slide in 0..total {
                let segments = self.deck.slides[slide].segments();
                for (index, segment) in segments.iter().enumerate() {
                    if let Segment::Tty(component) = segment
                        && component.prerender
                    {
                        self.manager
                            .prerender(slide, index, component, pre_rows, pre_cols);
                    }
                }
            }
        }

        // `usize::MAX` forces the first iteration to reconcile (spawn slide 0).
        let mut reconciled_for = usize::MAX;
        // Last (slide, cols, rows) painted; gates repaints so an idle slide
        // doesn't re-render every tick.
        let mut drawn: Option<(usize, u16, u16)> = None;

        loop {
            let segments = self.deck.slides[self.current].segments();
            if reconciled_for != self.current {
                self.manager.reconcile(self.current, &segments);
                reconciled_for = self.current;
                self.slide_started = Instant::now();
                drawn = None;
            }

            let Size {
                width: cols,
                height: rows,
            } = terminal.size().unwrap_or(Size::new(80, 24));
            let content_w = content_width(cols);
            let content_rows = rows.saturating_sub(FOOTER_ROWS);
            let content_cols = u16::try_from(content_w).unwrap_or(u16::MAX);
            self.manager
                .ensure(self.current, &segments, content_rows, content_cols);
            self.refresh_static_render(&segments, content_w);

            let content_area = Rect::new(0, 0, cols, content_rows);
            if drawn != Some((self.current, cols, rows)) || self.any_active(&segments, content_area) {
                let blink_on = (self.start.elapsed().as_millis() / BLINK_MS).is_multiple_of(2);
                terminal.draw(|frame| self.draw(frame, &segments, blink_on))?;
                drawn = Some((self.current, cols, rows));
            }

            if event::poll(TICK)? {
                let ev = event::read()?;
                if let Event::Key(key) = ev {
                    if key.kind == KeyEventKind::Press {
                        match action_for(&key) {
                            Some(Action::Next) => self.current = (self.current + 1).min(total - 1),
                            Some(Action::Prev) => self.current = self.current.saturating_sub(1),
                            Some(Action::First) => self.current = 0,
                            Some(Action::Last) => self.current = total - 1,
                            Some(Action::Quit) => return Ok(()),
                            None => {}
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
    use super::App;
    use super::parse_deck;
    use super::super::components::Cached;
    use ratatui::layout::Rect;

    #[test]
    fn static_render_caches_static_segments_not_processes() {
        let deck = parse_deck(
            "# H\n\n<mermaid>\nflowchart LR; A --> B\n</mermaid>\n\n<tty>\necho hi\n</tty>",
        )
        .unwrap();
        let segments = deck.slides[0].segments();
        let mut app = App::new(deck);

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
        let mut app = App::new(deck);

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
        let mut app = App::new(deck);
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
