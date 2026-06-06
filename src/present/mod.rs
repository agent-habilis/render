//! Full-screen presentation mode for `.ah-r` files.
//!
//! [`run`] parses the deck, takes over the terminal with [`ratatui::init`] (raw
//! mode + alternate screen + a restore-on-panic hook), and drives an [`App`]
//! loop. Each slide is split into [`Segment`]s: plain Markdown, plus the
//! embedded XML elements — `<code>` (statically syntax-highlighted via
//! [`crate::render::highlight_code`]), `<tty>` (a live process drawn into a
//! bordered box by [`tty`]), and `<mermaid>` (a drawn diagram). Arrow keys move
//! between slides; `q`/`Esc` quits.
//!
//! The loop is tick-driven rather than blocking: between key presses it repaints
//! so a running `<tty>` process streams live and the status indicator blinks.
//! Static segments (Markdown, Mermaid, and `<code>`) are rendered once per
//! slide+width and cached. Navigating reconciles the running processes (see
//! [`tty`]).

mod parse;
mod tty;

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect, Size};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use ansi_to_tui::IntoText;
use tui_term::widget::PseudoTerminal;

use crate::render;
use parse::{AnimationBlock, CodeBlock, Deck, Segment};
use tty::{ManagedProc, ProcessManager};

/// Horizontal margin (each side) kept clear of slide content.
const MARGIN: u16 = 4;
/// Widest a slide's content column ever grows, however wide the terminal is.
const MAX_CONTENT_WIDTH: usize = 100;
/// Narrowest content column we render to, even on a tiny terminal.
const MIN_CONTENT_WIDTH: usize = 20;
/// Rows reserved at the bottom for the footer (rule line + a blank row).
const FOOTER_ROWS: u16 = 2;
/// How long the event loop waits for input before ticking (to animate live
/// output and the blinking status indicator).
const TICK: Duration = Duration::from_millis(100);
/// Half-period of the running-status blink.
const BLINK_MS: u128 = 500;

/// Render a deck full-screen and navigate it until the user quits.
///
/// # Errors
/// Fails if the document contains no slides, or on a terminal I/O error while
/// drawing or reading input.
///
/// # Panics
/// Like any `ratatui` app, panics if entering full-screen mode fails (the
/// installed hook restores the terminal first). The caller has already checked
/// that both stdin and stdout are interactive terminals.
pub(crate) fn run(input: &str) -> Result<()> {
    let deck = parse::parse_deck(input);
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

/// The running presentation: the deck, the live processes, and the cursor.
#[derive(Debug)]
struct App {
    deck: Deck,
    manager: ProcessManager,
    current: usize,
    start: Instant,
    /// When the current slide became active — the clock for `<animation>`s
    /// (reset on every navigation, so a flipbook restarts when its slide opens).
    slide_started: Instant,
    static_render: StaticRender,
}

/// Memoized renders of the time-independent segments for the current slide.
/// These depend only on the segment source and the wrap width, so they are
/// reused across the (up to ~10 Hz) repaints a running process or animation
/// triggers, rather than re-running termimad/figurehead/syntect every frame.
#[derive(Debug, Default)]
struct StaticRender {
    /// The `(slide, content_w)` the cache is valid for.
    key: Option<(usize, usize)>,
    /// One entry per segment, by block index.
    items: Vec<Cached>,
}

/// A segment's cached render in [`StaticRender`].
#[derive(Debug)]
enum Cached {
    /// A live `<tty>` — laid out fresh each frame, nothing cached.
    Live,
    /// A static text render `(text, width, height)` (Markdown / Mermaid / Code).
    Static(Text<'static>, u16, u16),
    /// An `<animation>`'s frames, each pre-rendered `(text, width, height)`.
    Frames(Vec<(Text<'static>, u16, u16)>),
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

    /// The tick-driven render/navigation loop.
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let total = self.deck.slides.len();

        // Spawn `prerender` tty processes across all slides up front, sized to an
        // estimate of the content area, so a slow-starting program is already
        // drawn (and warmed up) by the time its slide is reached.
        {
            let size = terminal.size().unwrap_or(Size::new(80, 24));
            let pre_rows = size.height.saturating_sub(FOOTER_ROWS);
            let pre_cols = u16::try_from(content_width(size.width)).unwrap_or(u16::MAX);
            for slide in 0..total {
                let segments = self.deck.slides[slide].segments();
                for (block_index, segment) in segments.iter().enumerate() {
                    if let Segment::Tty(block) = segment
                        && block.prerender
                    {
                        self.manager
                            .prerender(slide, block_index, block, pre_rows, pre_cols);
                    }
                }
            }
        }

        // `usize::MAX` forces the first iteration to reconcile (spawn slide 0).
        let mut reconciled_for = usize::MAX;
        // Repaint only when the slide/size changed or a process is animating, so
        // an idle slide does not re-render markdown every tick.
        let mut drawn: Option<(usize, u16, u16)> = None;

        loop {
            let segments = self.deck.slides[self.current].segments();
            if reconciled_for != self.current {
                self.manager.reconcile(self.current, &segments);
                reconciled_for = self.current;
                self.slide_started = Instant::now(); // restart animation clocks
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

            // Repaint when a process is streaming or an animation is still
            // moving, in addition to slide/size changes.
            let active = self.manager.any_running(self.current, &segments)
                || self.any_animating(&segments);
            if drawn != Some((self.current, cols, rows)) || active {
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
                    drawn = None; // invalidate the last paint; the next iteration redraws
                }
            }
        }
    }

    /// Rebuild the [`StaticRender`] cache when the slide or wrap width changed;
    /// a no-op (one key comparison) on every other frame.
    fn refresh_static_render(&mut self, segments: &[Segment], content_w: usize) {
        let key = (self.current, content_w);
        if self.static_render.key == Some(key) {
            return;
        }
        let items = segments
            .iter()
            .map(|segment| match segment {
                Segment::Markdown(src) => {
                    let (text, width, height) = markdown_item(src, content_w);
                    Cached::Static(text, width, height)
                }
                Segment::Mermaid(src) => {
                    let (text, width, height) = mermaid_item(src);
                    Cached::Static(text, width, height)
                }
                Segment::Code(block) => {
                    let (text, width, height) = code_item(block);
                    Cached::Static(text, width, height)
                }
                Segment::Animation(block) => Cached::Frames(
                    block.frames.iter().map(|frame| frame_item(frame)).collect(),
                ),
                Segment::Tty(_) => Cached::Live,
            })
            .collect();
        self.static_render = StaticRender {
            key: Some(key),
            items,
        };
    }

    /// The active frame index for `block`, from the per-slide clock.
    fn frame_index(&self, block: &AnimationBlock) -> usize {
        let count = block.frames.len();
        if count <= 1 {
            return 0;
        }
        let elapsed_ms = self.slide_started.elapsed().as_millis();
        let raw = (elapsed_ms / u128::from(block.frame_ms.max(1))) as usize;
        if block.looping {
            raw % count
        } else {
            raw.min(count - 1)
        }
    }

    /// Whether any `<animation>` on the slide is still moving (so the loop keeps
    /// repainting): a looping multi-frame animation always is; a one-shot one
    /// only until it reaches its last frame.
    fn any_animating(&self, segments: &[Segment]) -> bool {
        segments.iter().any(|segment| match segment {
            Segment::Animation(block) => {
                block.frames.len() > 1
                    && (block.looping || self.frame_index(block) + 1 < block.frames.len())
            }
            Segment::Markdown(_) | Segment::Tty(_) | Segment::Code(_) | Segment::Mermaid(_) => false,
        })
    }

    /// Draw the current slide: a vertically-centered stack of its segments, with
    /// the footer below.
    fn draw(&self, frame: &mut Frame<'_>, segments: &[Segment], blink_on: bool) {
        let [content, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(FOOTER_ROWS)]).areas(frame.area());
        let content_w = content_width(content.width);

        let items = self.layout_items(segments, content, content_w);
        if !items.is_empty() {
            let constraints: Vec<Constraint> =
                items.iter().map(|item| Constraint::Length(item.height)).collect();
            let cells = Layout::vertical(constraints)
                .flex(Flex::Center)
                .spacing(1)
                .split(content);
            // One centered text column, sized to the widest text item; every
            // left-aligned item shares its left edge so prose and code line up.
            let column_width = items
                .iter()
                .filter(|item| item.left_align)
                .map(|item| item.width)
                .max()
                .unwrap_or(0);
            for (item, cell) in items.iter().zip(cells.iter()) {
                let area = if item.left_align {
                    let margin = cell.width.saturating_sub(column_width) / 2;
                    let width = item.width.min(cell.width.saturating_sub(margin));
                    Rect::new(cell.x + margin, cell.y, width, cell.height)
                } else {
                    let [centered] = Layout::horizontal([Constraint::Length(item.width)])
                        .flex(Flex::Center)
                        .areas(*cell);
                    centered
                };
                match &item.kind {
                    ItemKind::Markdown(text) => {
                        frame.render_widget(Paragraph::new(text.clone()), area);
                    }
                    ItemKind::Tty(block_index) => {
                        self.draw_tty(frame, area, segments, *block_index, blink_on);
                    }
                }
            }
        }

        self.draw_footer(frame, footer);
    }

    /// Measure each segment into an [`Item`], dropping empty Markdown.
    fn layout_items(&self, segments: &[Segment], content: Rect, content_w: usize) -> Vec<Item> {
        let mut items = Vec::new();
        for (block_index, segment) in segments.iter().enumerate() {
            match segment {
                // Static segments (Markdown, Mermaid, and syntax-highlighted
                // Code) are rendered once into `static_render` (keyed by slide +
                // width) and reused across repaints.
                Segment::Markdown(_) | Segment::Mermaid(_) | Segment::Code(_) => {
                    if let Some(Cached::Static(text, width, height)) =
                        self.static_render.items.get(block_index)
                        && *height > 0
                    {
                        items.push(Item {
                            width: (*width).min(content.width),
                            height: *height,
                            // Text (prose + code) shares one left-aligned column;
                            // diagrams stay centered on their own.
                            left_align: matches!(segment, Segment::Markdown(_) | Segment::Code(_)),
                            kind: ItemKind::Markdown(text.clone()),
                        });
                    }
                }
                // The active frame is picked from the cached frames by the
                // per-slide clock; the frames themselves are rendered once.
                Segment::Animation(block) => {
                    if let Some(Cached::Frames(frames)) = self.static_render.items.get(block_index)
                        && !frames.is_empty()
                    {
                        let (text, width, height) = &frames[self.frame_index(block)];
                        if *height > 0 {
                            items.push(Item {
                                width: (*width).min(content.width),
                                height: *height,
                                left_align: false, // ASCII art: centered as a block
                                kind: ItemKind::Markdown(text.clone()),
                            });
                        }
                    }
                }
                Segment::Tty(block) => {
                    let proc = self.manager.proc_for(self.current, block_index, block);
                    let (rows, cols) = proc.map_or_else(
                        || {
                            tty::box_interior(
                                block,
                                content.height.saturating_sub(2),
                                u16::try_from(content_w).unwrap_or(u16::MAX).saturating_sub(2),
                            )
                        },
                        ManagedProc::interior,
                    );
                    items.push(Item {
                        width: cols.saturating_add(2).min(content.width),
                        height: rows.saturating_add(2).min(content.height),
                        left_align: false,
                        kind: ItemKind::Tty(block_index),
                    });
                }
            }
        }
        items
    }

    /// Draw a single `tty` box: a bordered [`Block`] (title on top, status bar on
    /// the bottom) wrapping the process's live emulator screen.
    fn draw_tty(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        segments: &[Segment],
        block_index: usize,
        blink_on: bool,
    ) {
        let Some(Segment::Tty(block)) = segments.get(block_index) else {
            return;
        };
        let proc = self.manager.proc_for(self.current, block_index, block);
        let status = match (&block.status, proc) {
            (Some(fixed), _) => format!(" {fixed} "),
            (None, Some(proc)) => proc.status_line(blink_on),
            (None, None) => " starting… ".to_owned(),
        };
        let bordered = Block::bordered()
            .title(Line::from(format!(" {} ", block.display_title())))
            .title_bottom(Line::from(status).centered());

        match proc {
            Some(proc) => {
                let parser = proc.lock_parser();
                let pseudo = PseudoTerminal::new(parser.screen()).block(bordered);
                frame.render_widget(pseudo, area);
            }
            None => frame.render_widget(Paragraph::new("failed to start").block(bordered), area),
        }
    }

    /// Draw the man-style footer: a horizontal rule carrying the slide title on
    /// the left and `current/total` on the right. Both are titles on a top
    /// border, so `ratatui` handles their alignment and truncation.
    fn draw_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let title = self.deck.slides[self.current]
            .footer_title(&self.deck)
            .unwrap_or("");
        let page = format!("{}/{}", self.current + 1, self.deck.slides.len());
        // Render the rule on the first footer row, leaving the row below blank.
        let rule_area = Rect::new(area.x, area.y, area.width, 1);
        let footer = Block::new()
            .borders(Borders::TOP)
            .title(Line::from(title).left_aligned())
            .title(Line::from(page).right_aligned());
        frame.render_widget(footer, rule_area);
    }
}

/// One laid-out segment: its size and how to render it.
#[derive(Debug)]
struct Item {
    width: u16,
    height: u16,
    /// Share the left edge of the text column instead of being centered on its
    /// own. Set for all text (prose Markdown and `<code>`) so they line up
    /// flush-left as one column; `<mermaid>` diagrams and `<tty>` boxes stay
    /// centered.
    left_align: bool,
    kind: ItemKind,
}

#[derive(Debug)]
enum ItemKind {
    Markdown(Text<'static>),
    /// The segment index of a `tty` block (looked up again to render).
    Tty(usize),
}

/// A Markdown segment rendered (plainly — no embedded-diagram handling) to a
/// `ratatui` [`Text`] with its measured size.
fn markdown_item(src: &str, content_w: usize) -> (Text<'static>, u16, u16) {
    block_text(&render::markdown_plain(src, content_w))
}

/// A `<code>` segment syntax-highlighted to a `ratatui` [`Text`] with its
/// measured size. Borderless — just the highlighted code.
fn code_item(block: &CodeBlock) -> (Text<'static>, u16, u16) {
    block_text(&render::highlight_code(&block.lang, &block.code))
}

/// A `<mermaid>` segment drawn to a `ratatui` [`Text`] with its measured size,
/// falling back to the raw source when the diagram cannot be rendered.
fn mermaid_item(src: &str) -> (Text<'static>, u16, u16) {
    let rendered = render::mermaid(src).unwrap_or_else(|_| src.to_owned());
    block_text(&rendered)
}

/// An `<animation>` `<frame>` rendered **verbatim** (preformatted; every space
/// kept) to a `ratatui` [`Text`] with its measured size. Unlike [`block_text`],
/// surrounding blank lines are **not** trimmed, so a frame keeps its exact
/// height — the figure stays put across the flip instead of jumping.
fn frame_item(frame: &str) -> (Text<'static>, u16, u16) {
    let width = frame.lines().map(console::measure_text_width).max().unwrap_or(0);
    let height = frame.lines().count().max(1);
    let text = frame.into_text().unwrap_or_else(|_| Text::raw(frame.to_owned()));
    (
        text,
        u16::try_from(width).unwrap_or(u16::MAX),
        u16::try_from(height).unwrap_or(u16::MAX),
    )
}

/// Convert a rendered (ANSI) block to a `ratatui` [`Text`], returning it with its
/// measured display width and line count (surrounding blank lines trimmed so
/// vertical centering is not skewed).
fn block_text(rendered: &str) -> (Text<'static>, u16, u16) {
    let mut lines: Vec<&str> = rendered.split('\n').collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }

    let width = lines
        .iter()
        .map(|line| console::measure_text_width(line))
        .max()
        .unwrap_or(0);
    let height = lines.len();
    let joined = lines.join("\n");
    let text = match joined.as_str().into_text() {
        Ok(text) => text,
        Err(_) => Text::raw(joined),
    };

    (
        text,
        u16::try_from(width).unwrap_or(u16::MAX),
        u16::try_from(height).unwrap_or(u16::MAX),
    )
}

/// The Markdown wrap column for the current terminal width: the terminal minus
/// margins, clamped so content is neither edge-to-edge nor unreadably wide.
fn content_width(cols: u16) -> usize {
    let usable = usize::from(cols.saturating_sub(MARGIN * 2));
    usable.clamp(MIN_CONTENT_WIDTH, MAX_CONTENT_WIDTH)
}

/// A navigation intent decoded from a key press.
#[derive(Debug, Clone, Copy)]
enum Action {
    Next,
    Prev,
    First,
    Last,
    Quit,
}

/// Map a key press to a navigation action, or `None` to ignore it. `matches!`
/// keeps this off the non-exhaustive `KeyCode` enum's wildcard-match lint.
fn action_for(key: &KeyEvent) -> Option<Action> {
    use KeyCode as Code;

    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, Code::Char('c')) {
        return Some(Action::Quit);
    }
    if matches!(
        key.code,
        Code::Right | Code::Down | Code::PageDown | Code::Char(' ' | 'l' | 'j' | 'n')
    ) {
        return Some(Action::Next);
    }
    if matches!(
        key.code,
        Code::Left | Code::Up | Code::PageUp | Code::Backspace | Code::Char('h' | 'k' | 'p')
    ) {
        return Some(Action::Prev);
    }
    if matches!(key.code, Code::Home | Code::Char('g')) {
        return Some(Action::First);
    }
    if matches!(key.code, Code::End | Code::Char('G')) {
        return Some(Action::Last);
    }
    if matches!(key.code, Code::Char('q') | Code::Esc) {
        return Some(Action::Quit);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse::{Segment, parse_deck};
    use super::{App, Cached};

    #[test]
    fn static_render_caches_static_segments_not_processes() {
        // Slide: markdown heading, a <mermaid>, and a <tty> — in that order.
        let deck = parse_deck(
            "# H\n\n<mermaid>\nflowchart LR; A --> B\n</mermaid>\n\n<tty>\necho hi\n</tty>",
        );
        let segments = deck.slides[0].segments();
        let mut app = App::new(deck);

        app.refresh_static_render(&segments, 80);
        assert_eq!(app.static_render.key, Some((0, 80)));
        assert_eq!(app.static_render.items.len(), segments.len());
        // Markdown + Mermaid are cached; the live <tty> is not.
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

        // Same (slide, width): the key is retained (rebuild is skipped).
        app.refresh_static_render(&segments, 80);
        assert_eq!(app.static_render.key, Some((0, 80)));

        // A width change re-keys the cache.
        app.refresh_static_render(&segments, 60);
        assert_eq!(app.static_render.key, Some((0, 60)));
    }

    #[test]
    fn code_segment_is_cached_statically() {
        // A `<code>` block is highlighted statically now, so it caches like
        // Markdown rather than being a live (None) segment.
        let deck = parse_deck("<code lang=\"rust\">\nfn main() {}\n</code>");
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
        );
        let segments = deck.slides[0].segments();
        let mut app = App::new(deck);
        app.refresh_static_render(&segments, 80);

        // The animation caches its frames (one Frames entry, three frames).
        match &app.static_render.items[0] {
            Cached::Frames(frames) => assert_eq!(frames.len(), 3),
            Cached::Live | Cached::Static(..) => panic!("animation must cache frames"),
        }

        let Segment::Animation(block) = &segments[0] else {
            panic!("expected animation");
        };
        // Frame index advances with elapsed time and (here, non-looping) holds
        // the last frame; `any_animating` stays true until that last frame.
        let now = std::time::Instant::now();
        app.slide_started = now;
        assert_eq!(app.frame_index(block), 0);
        assert!(app.any_animating(&segments));
        app.slide_started = now.checked_sub(std::time::Duration::from_millis(250)).unwrap();
        assert_eq!(app.frame_index(block), 1);
        app.slide_started = now.checked_sub(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(app.frame_index(block), 2); // held at last (no loop)
        assert!(!app.any_animating(&segments)); // settled
    }
}
