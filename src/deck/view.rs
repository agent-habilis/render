use ansi_to_tui::IntoText;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::app::App;
use super::components::{Align, Item, Placement};
use super::segment::Segment;

const MARGIN: u16 = 4;
const MAX_CONTENT_WIDTH: usize = 100;
const MIN_CONTENT_WIDTH: usize = 20;
/// Footer text row plus a blank row beneath it.
pub(super) const FOOTER_ROWS: u16 = 2;

impl App {
    pub(super) fn draw(&self, frame: &mut Frame<'_>, segments: &[Segment], blink_on: bool) {
        let [content, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(FOOTER_ROWS)]).areas(frame.area());

        let laid = self.lay_out_segments(segments, content, blink_on);
        if !laid.is_empty() {
            let constraints: Vec<Constraint> =
                laid.iter().map(|(_, item)| Constraint::Length(item.height)).collect();
            let cells = Layout::vertical(constraints)
                .flex(Flex::Center)
                .spacing(1)
                .split(content);
            // Text-column items share one column, sized to the widest, so prose
            // and code line up flush-left while aligned items float.
            let column_width = laid
                .iter()
                .filter(|(_, item)| item.placement == Placement::Column)
                .map(|(_, item)| item.width)
                .max()
                .unwrap_or(0);
            for ((index, item), cell) in laid.iter().zip(cells.iter()) {
                let area = match item.placement {
                    Placement::Column => {
                        let margin = cell.width.saturating_sub(column_width) / 2;
                        let width = item.width.min(cell.width.saturating_sub(margin));
                        Rect::new(cell.x + margin, cell.y, width, cell.height)
                    }
                    Placement::Left => aligned(*cell, item.width, Align::Left),
                    Placement::Center => aligned(*cell, item.width, Align::Center),
                    Placement::Right => aligned(*cell, item.width, Align::Right),
                };
                let cached = &self.static_render.items[*index];
                let ctx = self.frame_ctx(cached, *index, content, blink_on);
                segments[*index].as_component().draw(frame, area, item, &ctx);
            }
        }

        self.draw_footer(frame, footer);
    }

    /// Returns each item with its segment index, so the draw pass can dispatch
    /// back to the same component.
    fn lay_out_segments(
        &self,
        segments: &[Segment],
        content: Rect,
        blink_on: bool,
    ) -> Vec<(usize, Item)> {
        segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| {
                let cached = self.static_render.items.get(index)?;
                let ctx = self.frame_ctx(cached, index, content, blink_on);
                Some((index, segment.as_component().lay_out(&ctx)?))
            })
            .collect()
    }

    /// A borderless footer: the deck title on the left and the page indicator on
    /// the right, separated by a flexible spacer. Both sides are rendered
    /// **as-is** — interpreting any ANSI escapes they carry — so a deck can pass
    /// fully formatted statusline pills (colors, Nerd Font glyphs) via its
    /// `title` and `pagination` attributes.
    fn draw_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let title = self.deck.title.as_deref().unwrap_or("");
        let cur = self.current + 1;
        let total_pages = self.deck.slides.len();
        let page = match self.deck.pagination.as_deref() {
            Some(template) => interpolate(template, cur, total_pages),
            None => format!("{cur}/{total_pages}"),
        };

        let (mut spans, left_width) = if title.is_empty() {
            (Vec::new(), 0)
        } else {
            (styled_spans(title), measure(title))
        };

        // A spacer pushes the page indicator flush right; the line is clipped
        // (page first to lose) only if it cannot fit on a very narrow terminal.
        let page_width = measure(&page);
        let total = usize::from(area.width);
        let gap = total.saturating_sub(left_width + page_width).max(1);
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(styled_spans(&page));

        // Draw on the footer's last row (a blank row stays above it for breathing
        // room) so the statusline sits flush at the bottom of the screen.
        let row = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
        frame.render_widget(Paragraph::new(Line::from(spans)), row);
    }
}

/// Parse a footer cell's ANSI into styled spans. Text with no escapes is just one
/// plain span; a parse failure falls back to the raw text rather than dropping it.
fn styled_spans(text: &str) -> Vec<Span<'static>> {
    text.into_text()
        .ok()
        .and_then(|parsed| parsed.lines.into_iter().next())
        .map_or_else(|| vec![Span::raw(text.to_owned())], |line| line.spans)
}

/// Rust-`format!`-style interpolation of the `pagination` template: `{cur}` and
/// `{total}` expand to the 1-based page and the slide count, `{{`/`}}` collapse
/// to single braces, and any other `{…}` (or stray brace) is kept verbatim. ANSI
/// escape sequences carry no braces, so they pass through untouched.
fn interpolate(template: &str, cur: usize, total: usize) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(idx) = rest.find(['{', '}']) {
        out.push_str(&rest[..idx]);
        let tail = &rest[idx..];
        if let Some(after) = tail.strip_prefix("{cur}") {
            out.push_str(&cur.to_string());
            rest = after;
        } else if let Some(after) = tail.strip_prefix("{total}") {
            out.push_str(&total.to_string());
            rest = after;
        } else if let Some(after) = tail.strip_prefix("{{") {
            out.push('{');
            rest = after;
        } else if let Some(after) = tail.strip_prefix("}}") {
            out.push('}');
            rest = after;
        } else {
            // A lone brace that starts no token: keep it and step past it.
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Display width in cells (ANSI escapes stripped) so the page indicator stays
/// flush right regardless of any formatting in a footer cell.
fn measure(text: &str) -> usize {
    console::measure_text_width(text)
}

/// Wrap column: terminal minus margins, clamped so content is neither
/// edge-to-edge nor unreadably wide.
pub(super) fn content_width(cols: u16) -> usize {
    let usable = usize::from(cols.saturating_sub(MARGIN * 2));
    usable.clamp(MIN_CONTENT_WIDTH, MAX_CONTENT_WIDTH)
}

/// Place a free-floating item of width `item_w` in `cell`: `Left`/`Right` hug the
/// edges of the centered content column; `Center` centers in the cell. The width
/// is clamped so the item never runs off the right edge.
fn aligned(cell: Rect, item_w: u16, align: Align) -> Rect {
    let content_w = u16::try_from(content_width(cell.width))
        .unwrap_or(u16::MAX)
        .min(cell.width);
    let col_x = cell.x + cell.width.saturating_sub(content_w) / 2;
    let start = match align {
        Align::Left => col_x,
        Align::Right => (col_x + content_w).saturating_sub(item_w),
        Align::Center => cell.x + cell.width.saturating_sub(item_w) / 2,
    };
    let width = item_w.min(cell.width.saturating_sub(start));
    Rect::new(start, cell.y, width, cell.height)
}

#[cfg(test)]
mod tests {
    use super::{Align, aligned, interpolate};
    use ratatui::layout::Rect;

    #[test]
    fn aligns_within_the_centered_content_column() {
        // 100 cols → content column 92, centered, so its left edge is at x=4.
        let cell = Rect::new(0, 0, 100, 5);
        assert_eq!(aligned(cell, 10, Align::Left).x, 4);
        assert_eq!(aligned(cell, 10, Align::Right).x, 86); // 4 + 92 - 10
        assert_eq!(aligned(cell, 10, Align::Center).x, 45); // centered in the cell
    }

    #[test]
    fn substitutes_cur_and_total() {
        assert_eq!(interpolate(" {cur}/{total} ", 2, 7), " 2/7 ");
        assert_eq!(interpolate("{total}", 1, 9), "9");
    }

    #[test]
    fn escapes_doubled_braces() {
        assert_eq!(interpolate("{{cur}} = {cur}", 3, 5), "{cur} = 3");
        assert_eq!(interpolate("{{}}", 1, 1), "{}");
    }

    #[test]
    fn keeps_unknown_placeholders_and_lone_braces() {
        assert_eq!(interpolate("{x} {cur}", 4, 8), "{x} 4");
        assert_eq!(interpolate("a { b } c", 1, 2), "a { b } c");
    }

    #[test]
    fn passes_ansi_through_untouched() {
        assert_eq!(
            interpolate("\x1b[1m{cur}\x1b[0m", 6, 10),
            "\x1b[1m6\x1b[0m"
        );
    }
}
