use std::path::Path;

use anyhow::{Context, Result};
use image::ImageReader;
use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::{TerminalOptions, Viewport};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

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

/// Detect the terminal's graphics protocol (kitty/iTerm2/Sixel), falling back to
/// halfblocks. `AH_R_NO_IMAGE_QUERY` skips the stdin query for terminals where it
/// misbehaves (and keeps tests from racing the read).
pub(crate) fn detect_picker() -> Picker {
    if std::env::var_os("AH_R_NO_IMAGE_QUERY").is_some() {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    }
}

/// The returned `Size` is the image's natural size in cells (pixels ÷ the
/// picker's font size).
pub(crate) fn load(picker: &Picker, path: &Path) -> Result<(Size, StatefulProtocol)> {
    let image = ImageReader::open(path)
        .with_context(|| format!("opening image `{}`", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("reading image `{}`", path.display()))?
        .decode()
        .with_context(|| format!("decoding image `{}`", path.display()))?;
    let natural = Resize::natural_size(&image, picker.font_size());
    let protocol = picker.new_resize_protocol(image);
    Ok((natural, protocol))
}

/// `value * mul / div`, in u32 to avoid overflow, clamped to `[1, u16::MAX]`.
pub(crate) fn scale(value: u16, mul: u16, div: u16) -> u16 {
    u16::try_from(u32::from(value) * u32::from(mul) / u32::from(div.max(1)))
        .unwrap_or(u16::MAX)
        .max(1)
}

/// The largest aspect-correct box (image proportions) fitting within
/// `max_w`×`max_h`. With `allow_upscale` it fills the box even when that means
/// growing past `natural`; otherwise it never exceeds the natural size
/// (shrink-only). Sizes the area that [`draw`] then fills.
pub(crate) fn fit_aspect(natural: Size, max_w: u16, max_h: u16, allow_upscale: bool) -> Size {
    let (nat_w, nat_h) = (natural.width.max(1), natural.height.max(1));
    let (max_w, max_h) = (max_w.max(1), max_h.max(1));
    let width = if allow_upscale {
        max_w
    } else {
        nat_w.min(max_w)
    };
    let by_width = Size::new(width, scale(width, nat_h, nat_w));
    if by_width.height <= max_h {
        by_width
    } else {
        Size::new(scale(max_h, nat_w, nat_h), max_h)
    }
}

/// Draw `protocol` into `area`, scaling the image (up or down) to fill it while
/// keeping aspect — the caller sizes `area` via [`fit_aspect`], so `Scale` only
/// upscales when an explicit larger box was requested.
pub(crate) fn draw(frame: &mut Frame<'_>, area: Rect, protocol: &mut StatefulProtocol) {
    let widget = StatefulImage::<StatefulProtocol>::default().resize(Resize::Scale(None));
    frame.render_stateful_widget(widget, area, protocol);
}

#[cfg(test)]
mod tests {
    use super::fit_aspect;
    use ratatui::layout::Size;

    #[test]
    fn shrink_only_never_upscales() {
        assert_eq!(
            fit_aspect(Size::new(100, 50), 40, 100, false),
            Size::new(40, 20)
        );
        // Height is the binding cap.
        assert_eq!(
            fit_aspect(Size::new(100, 50), 100, 10, false),
            Size::new(20, 10)
        );
        // Smaller than the box stays at natural size.
        assert_eq!(
            fit_aspect(Size::new(10, 6), 100, 100, false),
            Size::new(10, 6)
        );
    }

    #[test]
    fn upscale_fills_the_box_keeping_aspect() {
        assert_eq!(
            fit_aspect(Size::new(100, 50), 200, 200, true),
            Size::new(200, 100)
        );
        // A 30x30 box on a 2:1 image yields 30x15 (fit inside, not stretched).
        assert_eq!(
            fit_aspect(Size::new(100, 50), 30, 30, true),
            Size::new(30, 15)
        );
    }
}
