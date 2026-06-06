//! Drawing the current slide: a vertically-centered stack of laid-out
//! components plus the footer. Components measure and draw themselves through
//! the [`Component`](super::components::Component) trait; this module only does
//! the geometry.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders};

use super::app::App;
use super::components::Item;
use super::segment::Segment;

const MARGIN: u16 = 4;
const MAX_CONTENT_WIDTH: usize = 100;
const MIN_CONTENT_WIDTH: usize = 20;
/// Footer rule line plus a blank row beneath it.
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

    /// A man-page-style rule with the slide title left and `current/total` right.
    /// Both ride a top border so `ratatui` handles alignment and truncation.
    fn draw_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let title = self.deck.slides[self.current]
            .footer_title(&self.deck)
            .unwrap_or("");
        let page = format!("{}/{}", self.current + 1, self.deck.slides.len());
        let rule_area = Rect::new(area.x, area.y, area.width, 1);
        let footer = Block::new()
            .borders(Borders::TOP)
            .title(Line::from(title).left_aligned())
            .title(Line::from(page).right_aligned());
        frame.render_widget(footer, rule_area);
    }
}

/// Wrap column: terminal minus margins, clamped so content is neither
/// edge-to-edge nor unreadably wide.
pub(super) fn content_width(cols: u16) -> usize {
    let usable = usize::from(cols.saturating_sub(MARGIN * 2));
    usable.clamp(MIN_CONTENT_WIDTH, MAX_CONTENT_WIDTH)
}
