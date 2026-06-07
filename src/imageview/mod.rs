use std::path::Path;

use anyhow::Result;
use ratatui::layout::Rect;
use ratatui::{TerminalOptions, Viewport};

use crate::img::{detect_picker, draw, fit_aspect, load};

/// Render an image inline at the cursor and return, leaving it in the scrollback
/// (like `ah-r notes.md` prints and returns). The caller guarantees an
/// interactive TTY — the inline viewport's cursor-position query needs one.
pub(crate) fn render(path: &Path) -> Result<()> {
    // Build the picker before the inline terminal: its font size sets the
    // viewport height we must reserve up front.
    let picker = detect_picker();
    let (natural, mut protocol) = load(&picker, path)?;

    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    // Natural size, capped to the terminal (a row left for the prompt). No
    // upscale: with no size control, a small image stays its own size rather than
    // being blown up to fill the screen.
    let fitted = fit_aspect(natural, cols, rows.saturating_sub(1), false);

    // Inline viewport: raw mode, no alternate screen, so the image stays in the
    // scrollback when we restore.
    let mut terminal = ratatui::try_init_with_options(TerminalOptions {
        viewport: Viewport::Inline(fitted.height),
    })?;
    let result = terminal.draw(|frame| {
        // An inline viewport is anchored at the cursor's row, so draw from
        // `frame.area()`'s origin, not (0, 0), or the image is clipped away.
        let full = frame.area();
        let area = Rect {
            x: full.x,
            y: full.y,
            width: fitted.width.min(full.width),
            height: fitted.height.min(full.height),
        };
        draw(frame, area, &mut protocol);
    });
    ratatui::restore();
    // Move the shell prompt below the image rather than over its last row.
    println!();
    result?;
    Ok(())
}
