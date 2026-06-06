//! Full-screen presentation mode for `.ahr-deck` files.
//!
//! [`run`] parses the deck, takes over the terminal with [`ratatui::init`] (raw
//! mode + alternate screen + a restore-on-panic hook), and drives an [`App`]
//! loop. Each slide is split into [`Segment`]s: Markdown (rendered with
//! [`crate::render::markdown_to_width`], so embedded ` ```mermaid ` diagrams are
//! drawn inline) and ` ```tty ` blocks (live processes drawn into bordered boxes
//! by [`tty`]). Arrow keys move between slides; `q`/`Esc` quits.
//!
//! The loop is tick-driven rather than blocking: between key presses it repaints
//! so a running `tty` process streams its output live and its status indicator
//! blinks. Navigating reconciles the running processes (see [`tty`]).

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
use tui_term::widget::{Cursor, PseudoTerminal};

use crate::render;
use parse::{Deck, RenderMode, Segment};
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
}

impl App {
    fn new(deck: Deck) -> Self {
        Self {
            deck,
            manager: ProcessManager::new(),
            current: 0,
            start: Instant::now(),
        }
    }

    /// The tick-driven render/navigation loop.
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let total = self.deck.slides.len();

        // Eagerly spawn `render=eager` code editors across all slides so those
        // slides show already-highlighted code with no startup flash when reached.
        {
            let size = terminal.size().unwrap_or(Size::new(80, 24));
            let eager_rows = size.height.saturating_sub(FOOTER_ROWS);
            let eager_cols = u16::try_from(content_width(size.width)).unwrap_or(u16::MAX);
            for slide in 0..total {
                let segments = self.deck.slides[slide].segments();
                for (block_index, segment) in segments.iter().enumerate() {
                    if let Segment::Code(block) = segment
                        && block.render == RenderMode::Eager
                    {
                        self.manager
                            .ensure_code(slide, block_index, block, eager_rows, eager_cols);
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
                drawn = None;
            }

            let Size {
                width: cols,
                height: rows,
            } = terminal.size().unwrap_or(Size::new(80, 24));
            let content_rows = rows.saturating_sub(FOOTER_ROWS);
            let content_cols = u16::try_from(content_width(cols)).unwrap_or(u16::MAX);
            self.manager
                .ensure(self.current, &segments, content_rows, content_cols);

            let running = self.manager.any_running(self.current, &segments);
            if drawn != Some((self.current, cols, rows)) || running {
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
            for (item, cell) in items.iter().zip(cells.iter()) {
                let [centered] = Layout::horizontal([Constraint::Length(item.width)])
                    .flex(Flex::Center)
                    .areas(*cell);
                match &item.kind {
                    ItemKind::Markdown(text) => {
                        frame.render_widget(Paragraph::new(text.clone()), centered);
                    }
                    ItemKind::Tty(block_index) => {
                        self.draw_tty(frame, centered, segments, *block_index, blink_on);
                    }
                    ItemKind::Code(block_index) => {
                        self.draw_code(frame, centered, segments, *block_index);
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
                Segment::Markdown(src) => {
                    let (text, width, height) = markdown_item(src, content_w);
                    if height > 0 {
                        items.push(Item {
                            width: width.min(content.width),
                            height,
                            kind: ItemKind::Markdown(text),
                        });
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
                        kind: ItemKind::Tty(block_index),
                    });
                }
                Segment::Code(block) => {
                    // Borderless ("just the code"): the box is the editor interior.
                    let proc = self.manager.code_proc(self.current, block_index);
                    let (rows, cols) = proc.map_or_else(
                        || {
                            tty::code_interior(
                                &block.code,
                                content.height,
                                u16::try_from(content_w).unwrap_or(u16::MAX),
                            )
                        },
                        ManagedProc::interior,
                    );
                    items.push(Item {
                        width: cols.min(content.width),
                        height: rows.min(content.height),
                        kind: ItemKind::Code(block_index),
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

    /// Draw a code block: the editor's live screen, **borderless** — just the
    /// syntax-highlighted code. Falls back to the raw code when no editor
    /// spawned (e.g. `nvim` is not installed).
    fn draw_code(&self, frame: &mut Frame<'_>, area: Rect, segments: &[Segment], block_index: usize) {
        let Some(Segment::Code(block)) = segments.get(block_index) else {
            return;
        };
        match self.manager.code_proc(self.current, block_index) {
            Some(proc) => {
                let parser = proc.lock_parser();
                // Hide the editor's cursor: "just the code", no blinking block
                // and no visible jump from the startup dismiss keystrokes.
                let pseudo = PseudoTerminal::new(parser.screen())
                    .cursor(Cursor::default().visibility(false));
                frame.render_widget(pseudo, area);
            }
            None => frame.render_widget(Paragraph::new(block.code.clone()), area),
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
    kind: ItemKind,
}

#[derive(Debug)]
enum ItemKind {
    Markdown(Text<'static>),
    /// The segment index of a `tty` block (looked up again to render).
    Tty(usize),
    /// The segment index of a code block (looked up again to render).
    Code(usize),
}

/// Render a Markdown source to an ANSI string, then to a `ratatui` [`Text`],
/// returning it with its measured display width and line count (surrounding
/// blank lines trimmed so vertical centering is not skewed).
fn markdown_item(src: &str, content_w: usize) -> (Text<'static>, u16, u16) {
    let rendered = render::markdown_to_width(src, content_w);
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
