//! Full-screen presentation mode for `.ahr-deck` files.
//!
//! [`run`] parses the deck, takes over the terminal (raw mode + alternate
//! screen), and drives an event loop: each slide's Markdown is rendered with
//! [`crate::render::markdown_to_width`] (so embedded ` ```mermaid ` diagrams are
//! drawn inline), centered both ways by [`layout`], and topped with a man-style
//! footer. Arrow keys move between slides; `q`/`Esc` quits.

mod layout;
mod parse;

use std::io::{self, Write};

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::Print;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use crossterm::{cursor, execute, queue};

use crate::render;
use parse::Deck;

/// Horizontal margin (each side) kept clear of slide content.
const MARGIN: u16 = 4;
/// Widest a slide's content column ever grows, however wide the terminal is.
const MAX_CONTENT_WIDTH: usize = 100;
/// Narrowest content column we render to, even on a tiny terminal.
const MIN_CONTENT_WIDTH: usize = 20;

/// Render a deck full-screen and navigate it until the user quits.
///
/// # Errors
/// Fails if the document contains no slides, or on a terminal I/O error while
/// entering full-screen mode, drawing, or reading input.
pub(crate) fn run(input: &str) -> Result<()> {
    let deck = parse::parse_deck(input);
    if deck.slides.is_empty() {
        bail!("no slides found: separate slides with `---` lines");
    }

    install_panic_hook();
    let _guard = TerminalGuard::enter().context("entering full-screen presentation mode")?;
    let mut stdout = io::stdout();

    let total = deck.slides.len();
    let mut current = 0;
    // Only repaint when the slide or the terminal size actually changed, so an
    // unmapped key does not trigger a full clear + re-render (flicker + wasted
    // markdown/diagram work).
    let mut drawn: Option<(usize, u16, u16)> = None;
    loop {
        let (cols, rows) = terminal_size();
        if drawn != Some((current, cols, rows)) {
            draw(&mut stdout, &deck, current, cols, rows).context("drawing slide")?;
            drawn = Some((current, cols, rows));
        }

        let event = event::read().context("reading terminal event")?;
        if let Event::Key(key) = event {
            // Ignore key-release/repeat events (some platforms emit them) so a
            // single press advances a single slide.
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match action_for(&key) {
                Some(Action::Next) => current = (current + 1).min(total - 1),
                Some(Action::Prev) => current = current.saturating_sub(1),
                Some(Action::First) => current = 0,
                Some(Action::Last) => current = total - 1,
                Some(Action::Quit) => break,
                None => {}
            }
        }
        // Resize and other events fall through: the next iteration repaints only
        // if the size changed.
    }

    Ok(())
}

/// The terminal size, falling back to a sane default when it is unavailable or
/// reported as zero (some terminals/PTYs do this transiently) — mirroring
/// `render::wrap_width`'s 80-column fallback rather than rendering at width 0.
fn terminal_size() -> (u16, u16) {
    match size() {
        Ok((cols, rows)) if cols > 0 && rows > 0 => (cols, rows),
        Ok(_) | Err(_) => (80, 24),
    }
}

/// Clear the screen and draw slide `index` centered, with the footer.
fn draw(
    stdout: &mut io::Stdout,
    deck: &Deck,
    index: usize,
    cols: u16,
    rows: u16,
) -> io::Result<()> {
    let slide = &deck.slides[index];
    let rendered = render::markdown_to_width(&slide.body, content_width(cols));
    let title = slide.footer_title(deck).unwrap_or("");
    let frame = layout::render_frame(&rendered, cols, rows, title, index + 1, deck.slides.len());

    queue!(stdout, Clear(ClearType::All))?;
    for line in frame {
        queue!(stdout, cursor::MoveTo(line.col, line.row), Print(line.text))?;
    }
    stdout.flush()
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

/// Owns the terminal's full-screen state. Entering turns on raw mode, switches
/// to the alternate screen, and hides the cursor; [`Drop`] reverses all three
/// so a normal exit or an error (`?`) always restores the user's terminal.
#[derive(Debug)]
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort restore; nothing useful to do if these fail on the way out.
        let _ = execute!(io::stdout(), cursor::Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Restore the terminal on panic too. The release profile uses `panic = abort`,
/// which skips `Drop`, so without this a panic would leave the terminal in raw
/// mode on the alternate screen. The hook runs before aborting, then chains to
/// the previous hook to print the panic message.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), cursor::Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        previous(info);
    }));
}
