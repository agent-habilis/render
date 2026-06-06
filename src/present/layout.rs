//! Pure layout for a slide: place the rendered (already styled) content
//! centered both ways, and compose the man-style footer. Nothing here touches
//! the terminal, so it is straightforward to unit-test.
//!
//! Widths are measured with [`console::measure_text_width`], which ignores ANSI
//! escapes and counts wide Unicode, so the styled output from `termimad` and
//! the box-drawing diagrams from `figurehead` center correctly.

/// One ready-to-draw line: where to move the cursor and what to print there.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PositionedLine {
    pub(crate) row: u16,
    pub(crate) col: u16,
    pub(crate) text: String,
}

/// Rows reserved at the bottom for the footer (a rule line and the text line).
const FOOTER_ROWS: u16 = 2;

/// Compose a full frame: centered content above, footer below.
pub(crate) fn render_frame(
    rendered: &str,
    cols: u16,
    rows: u16,
    title: &str,
    current: usize,
    total: usize,
) -> Vec<PositionedLine> {
    let content_rows = rows.saturating_sub(FOOTER_ROWS);
    let mut frame = center_block(rendered, cols, content_rows);
    frame.extend(footer(cols, rows, title, current, total));
    frame
}

/// Center `rendered` as a block within a `cols` x `rows` area. The block (not
/// each line individually) is centered, so left edges stay aligned — important
/// for code blocks and diagrams.
fn center_block(rendered: &str, cols: u16, rows: u16) -> Vec<PositionedLine> {
    // Trim surrounding blank lines so vertical centering is not skewed by the
    // leading/trailing newlines the renderer leaves around content.
    let mut content: Vec<&str> = rendered.split('\n').collect();
    while content.last().is_some_and(|line| line.trim().is_empty()) {
        content.pop();
    }
    let leading = content
        .iter()
        .take_while(|line| line.trim().is_empty())
        .count();
    let body = &content[leading..];

    if body.is_empty() || rows == 0 {
        return Vec::new();
    }

    let block_width = body
        .iter()
        .map(|line| console::measure_text_width(line))
        .max()
        .unwrap_or(0);
    let left = usize::from(cols).saturating_sub(block_width) / 2;
    let top = usize::from(rows).saturating_sub(body.len()) / 2;
    // Columns available from `left` to the right edge. Anything wider (a wide
    // mermaid diagram, a long code line, or content on a very narrow terminal)
    // must be clipped, or the terminal soft-wraps it and wrecks the layout.
    let avail = usize::from(cols).saturating_sub(left);

    let mut lines = Vec::with_capacity(body.len());
    for (offset, line) in body.iter().enumerate() {
        let row = top + offset;
        if row >= usize::from(rows) {
            break; // content taller than the area: clip the overflow
        }
        lines.push(PositionedLine {
            row: to_u16(row),
            col: to_u16(left),
            text: clip_to_width(line, avail),
        });
    }
    lines
}

/// Truncate a styled line to `width` display columns. If the cut happens inside
/// a styled span, append a reset so the cut style does not bleed into the
/// footer; lines that already fit are returned untouched.
fn clip_to_width(line: &str, width: usize) -> String {
    if console::measure_text_width(line) <= width {
        return line.to_owned();
    }
    let mut clipped = console::truncate_str(line, width, "").into_owned();
    clipped.push_str("\u{1b}[0m");
    clipped
}

/// The man-style footer: a horizontal rule, then the current slide title on the
/// left and `current/total` on the right. On a terminal too short for both rows
/// (`rows < FOOTER_ROWS`) the rule is dropped so the status line still shows.
fn footer(cols: u16, rows: u16, title: &str, current: usize, total: usize) -> Vec<PositionedLine> {
    if rows == 0 {
        return Vec::new();
    }
    let width = usize::from(cols);
    let page = format!("{current}/{total}");
    let status = PositionedLine {
        row: rows - 1,
        col: 0,
        text: compose_status(width, title, &page),
    };
    if rows < FOOTER_ROWS {
        return vec![status];
    }
    vec![
        PositionedLine {
            row: rows - 2,
            col: 0,
            text: "─".repeat(width),
        },
        status,
    ]
}

/// Lay out the status line: `title` left-aligned, `page` right-aligned, padded
/// to `width` columns. The title is truncated (with an ellipsis) if it would
/// collide with the page counter.
fn compose_status(width: usize, title: &str, page: &str) -> String {
    let page_width = console::measure_text_width(page);
    if page_width >= width {
        return console::truncate_str(page, width, "").into_owned();
    }

    // Keep at least one space between the title and the page counter.
    let title_budget = width - page_width - 1;
    let title_fit = console::truncate_str(title, title_budget, "…");
    let title_width = console::measure_text_width(&title_fit);
    let gap = width - title_width - page_width;
    format!("{title_fit}{}{page}", " ".repeat(gap))
}

/// Clamp a `usize` position to `u16` for cursor moves. Positions derive from the
/// terminal size (already `u16`), so this never actually saturates in practice.
fn to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::{compose_status, render_frame};

    #[test]
    fn status_puts_title_left_and_page_right() {
        let line = compose_status(20, "Intro", "1/3");
        assert_eq!(line, "Intro            1/3");
        assert_eq!(line.chars().count(), 20);
    }

    #[test]
    fn status_truncates_a_long_title() {
        let line = compose_status(12, "A very long title", "2/9");
        assert_eq!(console::measure_text_width(&line), 12);
        assert!(line.ends_with("2/9"));
        assert!(line.contains('…'));
    }

    #[test]
    fn status_with_empty_title_right_aligns_page() {
        let line = compose_status(10, "", "5/5");
        assert_eq!(line, "       5/5");
    }

    #[test]
    fn over_wide_content_is_clipped_to_the_terminal_width() {
        // 10 cols; a 15-wide line must be clipped so it does not wrap.
        let frame = render_frame("0123456789abcde", 10, 6, "", 1, 1);
        let content = frame
            .iter()
            .find(|line| line.row < 4)
            .expect("content line");
        assert_eq!(content.col, 0);
        assert!(console::measure_text_width(&content.text) <= 10);
    }

    #[test]
    fn one_row_terminal_still_shows_a_status_line() {
        let frame = render_frame("hello", 20, 1, "T", 2, 5);
        assert_eq!(frame.len(), 1);
        assert_eq!(frame[0].row, 0);
        assert!(frame[0].text.ends_with("2/5"));
    }

    #[test]
    fn frame_centers_content_and_appends_footer() {
        // 10 cols x 6 rows: 4 content rows (rows - 2 footer), one content line.
        let frame = render_frame("hi", 10, 6, "T", 1, 2);
        // Footer rule on row 4, status on row 5.
        let rule = frame.iter().find(|line| line.row == 4).expect("rule row");
        assert_eq!(console::measure_text_width(&rule.text), 10);
        let status = frame.iter().find(|line| line.row == 5).expect("status row");
        assert!(status.text.ends_with("1/2"));
        // The single content line "hi" (width 2) is centered in 10 cols -> col 4,
        // and vertically centered in 4 rows -> row 1 or 2.
        let content = frame
            .iter()
            .find(|line| line.text == "hi")
            .expect("content line");
        assert_eq!(content.col, 4);
        assert!(content.row < 4);
    }
}
