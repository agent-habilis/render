//! Drawing the current slide: a vertically-centered stack of laid-out
//! components plus the footer. Components measure and draw themselves through
//! the [`Component`](super::components::Component) trait; this module only does
//! the geometry.

use ansi_to_tui::IntoText;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::app::App;
use super::components::Item;
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
            // Left-aligned items share one column, sized to the widest, so prose
            // and code line up flush-left while centered items float.
            let column_width = laid
                .iter()
                .filter(|(_, item)| item.left_align)
                .map(|(_, item)| item.width)
                .max()
                .unwrap_or(0);
            for ((index, item), cell) in laid.iter().zip(cells.iter()) {
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

    /// A borderless footer: the slide title on the left and `current/total` on
    /// the right, separated by a flexible spacer. The title is rendered **as-is**
    /// — interpreting any ANSI escapes it carries — so a deck can pass a fully
    /// formatted statusline (colors, Nerd Font glyphs) straight from its `title`.
    fn draw_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let title = self.deck.slides[self.current]
            .footer_title(&self.deck)
            .unwrap_or("");
        let page = format!("{}/{}", self.current + 1, self.deck.slides.len());

        let (mut spans, left_width) = if title.is_empty() {
            (Vec::new(), 0)
        } else {
            (title_spans(title), measure(title))
        };

        // A spacer pushes the page indicator flush right; both shrink before they
        // would overlap on a narrow terminal.
        let page_width = measure(&page);
        let total = usize::from(area.width);
        let gap = total.saturating_sub(left_width + page_width).max(1);
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::raw(page));

        // Draw on the footer's last row (a blank row stays above it for breathing
        // room) so the statusline sits flush at the bottom of the screen.
        let row = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
        frame.render_widget(Paragraph::new(Line::from(spans)), row);
    }
}

/// Parse the title's ANSI into styled spans. A title with no escapes is just one
/// plain span; a parse failure falls back to the raw text rather than dropping it.
fn title_spans(title: &str) -> Vec<Span<'static>> {
    title
        .into_text()
        .ok()
        .and_then(|text| text.lines.into_iter().next())
        .map_or_else(|| vec![Span::raw(title.to_owned())], |line| line.spans)
}

/// Display width in cells (ANSI escapes stripped) so the page indicator stays
/// flush right regardless of any formatting in the title.
fn measure(text: &str) -> usize {
    console::measure_text_width(text)
}

/// Wrap column: terminal minus margins, clamped so content is neither
/// edge-to-edge nor unreadably wide.
pub(super) fn content_width(cols: u16) -> usize {
    let usable = usize::from(cols.saturating_sub(MARGIN * 2));
    usable.clamp(MIN_CONTENT_WIDTH, MAX_CONTENT_WIDTH)
}
