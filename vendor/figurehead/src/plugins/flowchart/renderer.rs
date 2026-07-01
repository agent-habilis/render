//! ASCII rendering implementation for flowcharts
//!
//! Converts positioned nodes into ASCII diagrams using various character sets.

use anyhow::Result;
use tracing::{debug, info, span, trace, Level};

use super::{FlowchartDatabase, FlowchartLayoutAlgorithm, PositionedNode, PositionedSubgraph};
use crate::core::{
    wrap_label, AsciiCanvas, BoxChars, CharacterSet, Database, DiamondStyle, EdgeType,
    LayoutAlgorithm, NodeShape, Renderer,
};

/// Flowchart ASCII renderer
pub struct FlowchartRenderer {
    style: CharacterSet,
    diamond_style: DiamondStyle,
}

/// Max label width before wrapping (must match layout config)
const MAX_LABEL_WIDTH: usize = 30;

impl FlowchartRenderer {
    /// Create a new renderer with default Unicode style and Box diamond
    pub fn new() -> Self {
        Self {
            style: CharacterSet::Unicode,
            diamond_style: DiamondStyle::Box,
        }
    }

    /// Create a new renderer with a specific character set
    pub fn with_style(style: CharacterSet) -> Self {
        Self {
            style,
            diamond_style: DiamondStyle::Box,
        }
    }

    /// Create a new renderer with specific character set and diamond style
    pub fn with_styles(style: CharacterSet, diamond_style: DiamondStyle) -> Self {
        Self {
            style,
            diamond_style,
        }
    }

    /// Create a new renderer with a render config
    pub fn with_config(config: crate::core::RenderConfig) -> Self {
        Self {
            style: config.style,
            diamond_style: config.diamond_style,
        }
    }

    /// Get the current character set
    pub fn style(&self) -> CharacterSet {
        self.style
    }

    /// Get the current diamond style
    pub fn diamond_style(&self) -> DiamondStyle {
        self.diamond_style
    }

    fn draw_node(
        &self,
        canvas: &mut AsciiCanvas,
        node: &PositionedNode,
        shape: NodeShape,
        label: &str,
    ) {
        match shape {
            NodeShape::Rectangle => {
                self.draw_rectangle(canvas, node, label, BoxChars::rectangle(self.style))
            }
            NodeShape::Subroutine => self.draw_subroutine(canvas, node, label),
            NodeShape::RoundedRect => {
                self.draw_rectangle(canvas, node, label, BoxChars::rounded(self.style))
            }
            NodeShape::Diamond => self.draw_diamond(canvas, node, label),
            NodeShape::Circle | NodeShape::Terminal => self.draw_circle(canvas, node, label),
            NodeShape::Hexagon => self.draw_hexagon(canvas, node, label),
            NodeShape::Asymmetric => self.draw_asymmetric(canvas, node, label),
            NodeShape::Cylinder => self.draw_cylinder(canvas, node, label),
            NodeShape::Parallelogram => self.draw_parallelogram(canvas, node, label),
            NodeShape::Trapezoid => self.draw_trapezoid(canvas, node, label),
        }
    }

    /// Draw a subgraph boundary with centered title
    fn draw_subgraph(&self, canvas: &mut AsciiCanvas, subgraph: &PositionedSubgraph) {
        use unicode_width::UnicodeWidthStr;

        let chars = BoxChars::double(self.style);
        let x = subgraph.x;
        let y = subgraph.y;
        let w = subgraph.width;
        let h = subgraph.height;

        if w < 2 || h < 2 {
            return; // Too small to draw
        }

        // Calculate title positioning - center title in top border
        let title = &subgraph.title;
        let title_width = UnicodeWidthStr::width(title.as_str());

        // Format: ┌─── Title ───┐
        // We need at least 3 chars on each side for the pattern
        let total_dashes = w.saturating_sub(2); // excluding corners
        let title_with_padding = if title_width + 4 <= total_dashes {
            // Format: "── Title ──"
            let remaining = total_dashes.saturating_sub(title_width + 2); // 2 for spaces around title
            let left_dashes = remaining / 2;
            let right_dashes = remaining - left_dashes;

            let dash_char =
                if self.style == CharacterSet::Ascii || self.style == CharacterSet::Compact {
                    '='
                } else {
                    '═'
                };

            format!(
                "{} {} {}",
                std::iter::repeat_n(dash_char, left_dashes).collect::<String>(),
                title,
                std::iter::repeat_n(dash_char, right_dashes).collect::<String>()
            )
        } else {
            // Title too long, truncate if needed
            let truncated: String = title.chars().take(total_dashes.saturating_sub(2)).collect();
            format!(" {} ", truncated)
        };

        // Top border with title
        canvas.set_char(x, y, chars.top_left);
        for (i, c) in title_with_padding.chars().enumerate() {
            if i + 1 < w - 1 {
                canvas.set_char(x + 1 + i, y, c);
            }
        }
        // Fill remaining with horizontal line
        for i in (1 + title_with_padding.chars().count())..w - 1 {
            canvas.set_char(x + i, y, chars.horizontal);
        }
        canvas.set_char(x + w - 1, y, chars.top_right);

        // Left and right borders (only - don't fill interior)
        for row in 1..h - 1 {
            canvas.set_char(x, y + row, chars.vertical);
            canvas.set_char(x + w - 1, y + row, chars.vertical);
        }

        // Bottom border
        canvas.set_char(x, y + h - 1, chars.bottom_left);
        for i in 1..w - 1 {
            canvas.set_char(x + i, y + h - 1, chars.horizontal);
        }
        canvas.set_char(x + w - 1, y + h - 1, chars.bottom_right);
    }

    /// Redraw only the title portion of a subgraph's top border
    /// Called after edges to fix overlap issues
    fn redraw_subgraph_title(&self, canvas: &mut AsciiCanvas, subgraph: &PositionedSubgraph) {
        use unicode_width::UnicodeWidthStr;

        let chars = BoxChars::double(self.style);
        let x = subgraph.x;
        let y = subgraph.y;
        let w = subgraph.width;

        if w < 2 {
            return;
        }

        let title = &subgraph.title;
        let title_width = UnicodeWidthStr::width(title.as_str());

        let total_dashes = w.saturating_sub(2);
        let title_with_padding = if title_width + 4 <= total_dashes {
            let remaining = total_dashes.saturating_sub(title_width + 2);
            let left_dashes = remaining / 2;
            let right_dashes = remaining - left_dashes;

            let dash_char =
                if self.style == CharacterSet::Ascii || self.style == CharacterSet::Compact {
                    '='
                } else {
                    '═'
                };

            format!(
                "{} {} {}",
                std::iter::repeat_n(dash_char, left_dashes).collect::<String>(),
                title,
                std::iter::repeat_n(dash_char, right_dashes).collect::<String>()
            )
        } else {
            let truncated: String = title.chars().take(total_dashes.saturating_sub(2)).collect();
            format!(" {} ", truncated)
        };

        // Redraw top border with title
        canvas.set_char(x, y, chars.top_left);
        for (i, c) in title_with_padding.chars().enumerate() {
            if i + 1 < w - 1 {
                canvas.set_char(x + 1 + i, y, c);
            }
        }
        for i in (1 + title_with_padding.chars().count())..w - 1 {
            canvas.set_char(x + i, y, chars.horizontal);
        }
        canvas.set_char(x + w - 1, y, chars.top_right);
    }

    fn draw_rectangle(
        &self,
        canvas: &mut AsciiCanvas,
        node: &PositionedNode,
        label: &str,
        chars: BoxChars,
    ) {
        use unicode_width::UnicodeWidthStr;

        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        // Top border
        canvas.set_char(x, y, chars.top_left);
        for i in 1..w - 1 {
            canvas.set_char(x + i, y, chars.horizontal);
        }
        canvas.set_char(x + w - 1, y, chars.top_right);

        // Sides and content
        for row in 1..h - 1 {
            canvas.set_char(x, y + row, chars.vertical);
            canvas.set_char(x + w - 1, y + row, chars.vertical);
        }

        // Wrap and draw label(s) centered vertically and horizontally
        let lines = wrap_label(label, MAX_LABEL_WIDTH);
        let total_lines = lines.len();
        let start_y = y + (h.saturating_sub(total_lines)) / 2;

        for (i, line) in lines.iter().enumerate() {
            let line_width = UnicodeWidthStr::width(line.as_str());
            let label_x = x + (w.saturating_sub(line_width)) / 2;
            let label_y = start_y + i;
            if label_y > y && label_y < y + h - 1 {
                canvas.draw_text(label_x.max(x + 1), label_y, line);
            }
        }

        // Bottom border
        canvas.set_char(x, y + h - 1, chars.bottom_left);
        for i in 1..w - 1 {
            canvas.set_char(x + i, y + h - 1, chars.horizontal);
        }
        canvas.set_char(x + w - 1, y + h - 1, chars.bottom_right);
    }

    fn draw_subroutine(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        self.draw_rectangle(canvas, node, label, BoxChars::rectangle(self.style));

        // Add the inner vertical lines that characterize subroutines
        if node.width > 4 && node.height > 2 {
            let left = node.x + 1;
            let right = node.x + node.width - 2;
            for row in node.y + 1..node.y + node.height - 1 {
                let pipe = if self.style.is_ascii() { '|' } else { '│' };
                canvas.set_char(left, row, pipe);
                canvas.set_char(right, row, pipe);
            }
        }
    }

    fn draw_hexagon(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        // Top slant
        if w > 2 {
            canvas.set_char(x + 1, y, '/');
            for i in x + 2..x + w - 2 {
                canvas.set_char(i, y, '-');
            }
            canvas.set_char(x + w - 2, y, '\\');
        }

        // Middle
        let mid_y = y + h / 2;
        canvas.set_char(x, mid_y, '<');
        canvas.set_char(x + w - 1, mid_y, '>');
        let label_x = x + (w.saturating_sub(label.len())) / 2;
        canvas.draw_text(label_x.max(x + 1), mid_y, label);

        // Upper and lower slants
        for row in 1..h - 1 {
            let current_y = y + row;
            if current_y == mid_y {
                continue;
            }
            canvas.set_char(x, current_y, '/');
            canvas.set_char(x + w - 1, current_y, '\\');
        }

        // Bottom slant
        if w > 2 {
            canvas.set_char(x + 1, y + h - 1, '\\');
            for i in x + 2..x + w - 2 {
                canvas.set_char(i, y + h - 1, '-');
            }
            canvas.set_char(x + w - 2, y + h - 1, '/');
        }
    }

    fn draw_asymmetric(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        // Start with a rectangle base
        self.draw_rectangle(canvas, node, label, BoxChars::rectangle(self.style));

        // Replace the right edge with an angled flag tip
        let mid_y = node.y + node.height / 2;
        let tip_x = node.x + node.width - 1;
        canvas.set_char(tip_x, mid_y, '>');
        if node.height > 2 {
            canvas.set_char(tip_x, node.y, '┐');
            canvas.set_char(tip_x, node.y + node.height - 1, '┘');
        }
    }

    fn draw_cylinder(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        // Top rim using braille dots for a softer curve
        let rim = if self.style.is_ascii() { '=' } else { '⠒' };
        let top_left = if self.style.is_ascii() { '+' } else { '╭' };
        let top_right = if self.style.is_ascii() { '+' } else { '╮' };
        canvas.set_char(x, y, top_left);
        for i in 1..w - 1 {
            canvas.set_char(x + i, y, rim);
        }
        canvas.set_char(x + w - 1, y, top_right);

        // Middle walls and label
        let label_y = y + h / 2;
        for row in 1..h - 1 {
            let wall = if self.style.is_ascii() { '|' } else { '│' };
            canvas.set_char(x, y + row, wall);
            canvas.set_char(x + w - 1, y + row, wall);
        }
        let label_x = x + (w.saturating_sub(label.len())) / 2;
        canvas.draw_text(label_x.max(x + 1), label_y, label);

        // Bottom rim mirrors top
        let bottom_left = if self.style.is_ascii() { '+' } else { '╰' };
        let bottom_right = if self.style.is_ascii() { '+' } else { '╯' };
        canvas.set_char(x, y + h - 1, bottom_left);
        for i in 1..w - 1 {
            canvas.set_char(x + i, y + h - 1, rim);
        }
        canvas.set_char(x + w - 1, y + h - 1, bottom_right);
    }

    fn draw_parallelogram(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        for row in 0..h {
            let row_y = y + row;
            let offset = row % 2; // small zig to hint slant without exceeding width
            let left_x = x + offset;
            let right_x = x + w - 1 - offset;

            // Fill top/bottom with angled ends
            if row == 0 {
                canvas.set_char(left_x, row_y, '/');
                for i in left_x + 1..right_x {
                    let line = if self.style.is_ascii() { '-' } else { '─' };
                    canvas.set_char(i, row_y, line);
                }
                canvas.set_char(right_x, row_y, '/');
            } else if row == h - 1 {
                canvas.set_char(left_x, row_y, '\\');
                for i in left_x + 1..right_x {
                    let line = if self.style.is_ascii() { '-' } else { '─' };
                    canvas.set_char(i, row_y, line);
                }
                canvas.set_char(right_x, row_y, '\\');
            } else {
                canvas.set_char(left_x, row_y, '/');
                canvas.set_char(right_x, row_y, '/');
            }

            if row == h / 2 {
                let label_x = left_x + (w.saturating_sub(label.len())) / 2;
                canvas.draw_text(label_x.max(left_x + 1), row_y, label);
            }
        }
    }

    fn draw_trapezoid(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        let top_padding = w.min(4) / 2;
        let span_char = if self.style.is_ascii() { '-' } else { '⠒' };

        // Top edge narrower
        let top_left = x + top_padding;
        let top_right = x + w - 1 - top_padding;
        canvas.set_char(top_left, y, '/');
        for i in top_left + 1..top_right {
            canvas.set_char(i, y, span_char);
        }
        canvas.set_char(top_right, y, '\\');

        // Sides
        for row in 1..h - 1 {
            let left_x = x + row.min(top_padding);
            let right_x = x + w - 1 - row.min(top_padding);
            canvas.set_char(left_x, y + row, '/');
            canvas.set_char(right_x, y + row, '\\');
            if row == h / 2 {
                let label_x = left_x + (right_x.saturating_sub(left_x) + 1 - label.len()) / 2;
                canvas.draw_text(label_x.max(left_x + 1), y + row, label);
            }
        }

        // Base
        let base_left = if self.style.is_ascii() { '+' } else { '└' };
        let base_right = if self.style.is_ascii() { '+' } else { '┘' };
        canvas.set_char(x, y + h - 1, base_left);
        for i in x + 1..x + w - 1 {
            let line = if self.style.is_ascii() { '-' } else { '─' };
            canvas.set_char(i, y + h - 1, line);
        }
        canvas.set_char(x + w - 1, y + h - 1, base_right);
    }

    fn draw_diamond(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        // First check diamond_style for Box and Inline
        match self.diamond_style {
            DiamondStyle::Box => {
                // Compact 3-line box with diamond corners:
                // ◆─────────◆
                // │ decide  │
                // ◆─────────◆
                let corner = if self.style.is_ascii() { '+' } else { '◆' };
                let horiz = if self.style.is_ascii() { '-' } else { '─' };
                let vert = if self.style.is_ascii() { '|' } else { '│' };

                // Top row
                canvas.set_char(x, y, corner);
                for i in x + 1..x + w - 1 {
                    canvas.set_char(i, y, horiz);
                }
                canvas.set_char(x + w - 1, y, corner);

                // Middle row(s) with label
                let mid_y = y + h / 2;
                canvas.set_char(x, mid_y, vert);
                canvas.set_char(x + w - 1, mid_y, vert);
                let label_x = x + (w.saturating_sub(label.len())) / 2;
                canvas.draw_text(label_x.max(x + 1), mid_y, label);

                // Bottom row
                canvas.set_char(x, y + h - 1, corner);
                for i in x + 1..x + w - 1 {
                    canvas.set_char(i, y + h - 1, horiz);
                }
                canvas.set_char(x + w - 1, y + h - 1, corner);
                return;
            }
            DiamondStyle::Inline => {
                // Minimal single-line inline style:
                // ◆ decide ◆
                let diamond = if self.style.is_ascii() { '<' } else { '◆' };
                let mid_y = y + h / 2;

                canvas.set_char(x, mid_y, diamond);
                let label_x = x + 2;
                canvas.draw_text(label_x, mid_y, label);
                canvas.set_char(x + w - 1, mid_y, diamond);
                return;
            }
            DiamondStyle::Tall => {
                // Fall through to CharacterSet-based rendering below
            }
        }

        // Tall diamond style - use CharacterSet-based rendering
        match self.style {
            CharacterSet::Compact => {
                // Compact diamond using box drawing diagonals ╱╲ (U+2571-2572):
                //   _╱╲_
                //   ◁ X ▷
                //   ‾╲╱‾
                let mid_y = y + h / 2;
                let center_x = x + w / 2;

                // Top row: underscores with ╱╲ cap
                for i in (x + 1)..center_x {
                    canvas.set_char(i, y, '_');
                }
                canvas.set_char(center_x, y, '╱');
                canvas.set_char(center_x + 1, y, '╲');
                for i in (center_x + 2)..(x + w - 1) {
                    canvas.set_char(i, y, '_');
                }

                // Middle row: triangle points and label
                canvas.set_char(x, mid_y, '◁');
                canvas.set_char(x + w - 1, mid_y, '▷');
                let label_x = x + (w.saturating_sub(label.len())) / 2;
                canvas.draw_text(label_x.max(x + 1), mid_y, label);

                // Bottom row: overlines with ╲╱ cap
                let bot_y = y + h - 1;
                for i in (x + 1)..center_x {
                    canvas.set_char(i, bot_y, '‾');
                }
                canvas.set_char(center_x, bot_y, '╲');
                canvas.set_char(center_x + 1, bot_y, '╱');
                for i in (center_x + 2)..(x + w - 1) {
                    canvas.set_char(i, bot_y, '‾');
                }
            }
            CharacterSet::UnicodeMath => {
                // UnicodeMath diamond with staggered ⟋⟍ diagonals and ⧼⧽ points
                //
                // Short (h <= 3): Compact bar style
                //   __⟋⟍__
                //   ⧼ X  ⧽
                //   ‾‾⟍⟋‾‾
                //
                // Tall (h > 3): Full staggered diagonals
                //       ⟋⟍
                //     ⟋    ⟍
                //    ⧼  X   ⧽
                //     ⟍    ⟋
                //       ⟍⟋
                let mid_y = y + h / 2;
                let center_x = x + w / 2;

                if h <= 3 {
                    // Compact: 3-row bar style with center cap
                    // Top bar: underscores with ⟋⟍ cap
                    for i in (x + 1)..center_x {
                        canvas.set_char(i, y, '_');
                    }
                    canvas.set_char(center_x, y, '⟋');
                    canvas.set_char(center_x + 1, y, '⟍');
                    for i in (center_x + 2)..(x + w - 1) {
                        canvas.set_char(i, y, '_');
                    }

                    // Middle row: brackets and label
                    canvas.set_char(x, mid_y, '⧼');
                    canvas.set_char(x + w - 1, mid_y, '⧽');
                    let label_x = x + (w.saturating_sub(label.len())) / 2;
                    canvas.draw_text(label_x.max(x + 1), mid_y, label);

                    // Bottom bar: overlines with ⟍⟋ cap
                    let bot_y = y + h - 1;
                    for i in (x + 1)..center_x {
                        canvas.set_char(i, bot_y, '‾');
                    }
                    canvas.set_char(center_x, bot_y, '⟍');
                    canvas.set_char(center_x + 1, bot_y, '⟋');
                    for i in (center_x + 2)..(x + w - 1) {
                        canvas.set_char(i, bot_y, '‾');
                    }
                } else {
                    // Tall: full staggered diagonals
                    let half_h = h / 2;

                    // Top point
                    canvas.set_char(center_x, y, '⟋');
                    canvas.set_char(center_x + 1, y, '⟍');

                    // Upper expanding rows - stagger 2 chars per row for shallower angle
                    for row in 1..half_h {
                        let offset = row * 2;
                        let left_x = center_x.saturating_sub(offset);
                        let right_x = center_x + 1 + offset;
                        canvas.set_char(left_x, y + row, '⟋');
                        canvas.set_char(right_x, y + row, '⟍');
                    }

                    // Middle row with brackets and label
                    canvas.set_char(x, mid_y, '⧼');
                    canvas.set_char(x + w - 1, mid_y, '⧽');
                    let label_x = x + (w.saturating_sub(label.len())) / 2;
                    canvas.draw_text(label_x.max(x + 1), mid_y, label);

                    // Lower contracting rows - staggered
                    for row in 1..half_h {
                        let offset = (half_h - row) * 2;
                        let left_x = center_x.saturating_sub(offset);
                        let right_x = center_x + 1 + offset;
                        canvas.set_char(left_x, mid_y + row, '⟍');
                        canvas.set_char(right_x, mid_y + row, '⟋');
                    }

                    // Bottom point
                    canvas.set_char(center_x, y + h - 1, '⟍');
                    canvas.set_char(center_x + 1, y + h - 1, '⟋');
                }
            }
            _ => {
                // Default ASCII/Unicode: steep /\ diagonals
                //     /\        row 0: top point
                //    /  \       row 1: expanding
                //   <text>      row 2: middle (widest, with label)
                //    \  /       row 3: contracting
                //     \/        row 4: bottom point
                let mid_y = y + h / 2;
                let half_h = h / 2;
                let center_x = x + w / 2;

                // Top point
                canvas.set_char(center_x, y, '/');
                canvas.set_char(center_x + 1, y, '\\');

                // Upper expanding rows (between top point and middle)
                for row in 1..half_h {
                    let left_x = center_x.saturating_sub(row);
                    let right_x = center_x + 1 + row;
                    canvas.set_char(left_x, y + row, '/');
                    canvas.set_char(right_x, y + row, '\\');
                }

                // Middle row with label
                canvas.set_char(x, mid_y, '<');
                canvas.set_char(x + w - 1, mid_y, '>');
                let label_x = x + (w.saturating_sub(label.len())) / 2;
                canvas.draw_text(label_x.max(x + 1), mid_y, label);

                // Lower contracting rows (between middle and bottom point)
                for row in 1..half_h {
                    let left_x = center_x.saturating_sub(half_h - row);
                    let right_x = center_x + 1 + (half_h - row);
                    canvas.set_char(left_x, mid_y + row, '\\');
                    canvas.set_char(right_x, mid_y + row, '/');
                }

                // Bottom point
                canvas.set_char(center_x, y + h - 1, '\\');
                canvas.set_char(center_x + 1, y + h - 1, '/');
            }
        }
    }

    fn draw_circle(&self, canvas: &mut AsciiCanvas, node: &PositionedNode, label: &str) {
        let x = node.x;
        let y = node.y;
        let w = node.width;
        let h = node.height;

        // Top - use parentheses for both ASCII and Unicode
        for i in 0..w {
            let ch = if i == 0 {
                '('
            } else if i == w - 1 {
                ')'
            } else {
                '-'
            };
            canvas.set_char(x + i, y, ch);
        }

        // Middle
        for row in 1..h - 1 {
            canvas.set_char(x, y + row, '(');
            canvas.set_char(x + w - 1, y + row, ')');
        }

        // Label
        let label_y = y + h / 2;
        let label_x = x + (w.saturating_sub(label.len())) / 2;
        canvas.draw_text(label_x.max(x + 1), label_y, label);

        // Bottom
        for i in 0..w {
            let ch = if i == 0 {
                '('
            } else if i == w - 1 {
                ')'
            } else {
                '-'
            };
            canvas.set_char(x + i, y + h - 1, ch);
        }
    }

    fn draw_edge(
        &self,
        canvas: &mut AsciiCanvas,
        waypoints: &[(usize, usize)],
        edge_type: EdgeType,
    ) {
        if waypoints.len() < 2 {
            return;
        }

        let chars = EdgeChars::for_type(edge_type, self.style);
        if chars.is_invisible() {
            return;
        }

        let has_arrow = edge_type.has_arrow();

        // Handle multi-segment waypoints (for back-edges)
        if waypoints.len() > 2 {
            self.draw_multi_segment_edge(canvas, waypoints, &chars, has_arrow);
            return;
        }

        let (x1, y1) = waypoints[0];
        let (x2, y2) = waypoints[waypoints.len() - 1];

        // Determine if we need orthogonal routing
        if y1 == y2 {
            // Pure horizontal - adjust endpoint for arrow
            let end_x = if has_arrow {
                if x2 > x1 {
                    x2.saturating_sub(1)
                } else {
                    x2 + 1
                }
            } else {
                x2
            };
            self.draw_horizontal_line(canvas, y1, x1, end_x, &chars);
            if has_arrow {
                let arrow = if x2 > x1 {
                    chars.arrow_right
                } else {
                    chars.arrow_left
                };
                canvas.set_char(end_x, y1, arrow);
            }
        } else if x1 == x2 {
            // Pure vertical - adjust endpoint for arrow
            let end_y = if has_arrow {
                if y2 > y1 {
                    y2.saturating_sub(1)
                } else {
                    y2 + 1
                }
            } else {
                y2
            };
            self.draw_vertical_line(canvas, x1, y1, end_y, &chars);
            if has_arrow {
                let arrow = if y2 > y1 {
                    chars.arrow_down
                } else {
                    chars.arrow_up
                };
                canvas.set_char(x1, end_y, arrow);
            }
        } else {
            // Orthogonal routing: vertical first, then horizontal for downward edges
            // This ensures proper connection from source node
            if y2 > y1 {
                // Going down: vertical first, then horizontal, then down to target
                // Turn point needs to be high enough to leave room for arrow segment
                let turn_y = y2.saturating_sub(2).max(y1 + 1); // At least 2 rows above target

                // Vertical segment from source down to turn point
                self.draw_vertical_line(canvas, x1, y1, turn_y, &chars);

                // Corner at (x1, turn_y)
                let corner1 = if self.style.is_ascii() {
                    '+'
                } else if x2 > x1 {
                    '└'
                } else {
                    '┘'
                };
                canvas.set_char(x1, turn_y, corner1);

                // Horizontal segment to target x
                self.draw_horizontal_line(canvas, turn_y, x1, x2, &chars);

                // Corner at (x2, turn_y)
                let corner2 = if self.style.is_ascii() {
                    '+'
                } else if x2 > x1 {
                    '┐'
                } else {
                    '┌'
                };
                canvas.set_char(x2, turn_y, corner2);

                // Vertical segment from corner to arrow position
                self.draw_vertical_line(canvas, x2, turn_y, y2, &chars);

                // Arrow pointing down into target
                if has_arrow {
                    canvas.set_char(x2, y2.saturating_sub(1), chars.arrow_down);
                }
            } else {
                // Going up: horizontal first, then vertical
                let turn_x = if x2 > x1 {
                    x2.saturating_sub(1)
                } else {
                    x2 + 1
                };

                // Horizontal segment to turn point
                self.draw_horizontal_line(canvas, y1, x1, turn_x, &chars);

                // Corner at turn point
                let corner = if self.style.is_ascii() {
                    '+'
                } else if x2 > x1 {
                    '┘'
                } else {
                    '└'
                };
                canvas.set_char(turn_x, y1, corner);

                // Vertical segment from corner toward target
                self.draw_vertical_line(canvas, turn_x, y2, y1, &chars);

                // Arrow pointing up into target
                if has_arrow {
                    canvas.set_char(turn_x, y2 + 1, chars.arrow_up);
                }
            }
        }
    }

    /// Draw a multi-segment edge (for back-edges routed around the diagram)
    fn draw_multi_segment_edge(
        &self,
        canvas: &mut AsciiCanvas,
        waypoints: &[(usize, usize)],
        chars: &EdgeChars,
        has_arrow: bool,
    ) {
        // Draw each segment between consecutive waypoints
        for i in 0..waypoints.len() - 1 {
            let (x1, y1) = waypoints[i];
            let (x2, y2) = waypoints[i + 1];
            let is_last = i == waypoints.len() - 2;

            if x1 == x2 {
                // Vertical segment
                let end_y = if is_last && has_arrow {
                    if y2 > y1 {
                        y2.saturating_sub(1)
                    } else {
                        y2 + 1
                    }
                } else {
                    y2
                };
                self.draw_vertical_line(canvas, x1, y1, end_y, chars);
            } else if y1 == y2 {
                // Horizontal segment
                let end_x = if is_last && has_arrow {
                    if x2 > x1 {
                        x2.saturating_sub(1)
                    } else {
                        x2 + 1
                    }
                } else {
                    x2
                };
                self.draw_horizontal_line(canvas, y1, x1, end_x, chars);
            }

            // Draw corner at waypoint (except at start and end)
            if i > 0 {
                let (prev_x, prev_y) = waypoints[i - 1];
                let corner = self.get_corner_char(prev_x, prev_y, x1, y1, x2, y2);
                canvas.set_char(x1, y1, corner);
            }
        }

        // Draw arrow at the end
        if has_arrow {
            let (x1, y1) = waypoints[waypoints.len() - 2];
            let (x2, y2) = waypoints[waypoints.len() - 1];
            let arrow = if x1 == x2 {
                if y2 > y1 {
                    chars.arrow_down
                } else {
                    chars.arrow_up
                }
            } else if x2 > x1 {
                chars.arrow_right
            } else {
                chars.arrow_left
            };
            let arrow_pos = if x1 == x2 {
                (
                    x2,
                    if y2 > y1 {
                        y2.saturating_sub(1)
                    } else {
                        y2 + 1
                    },
                )
            } else {
                (
                    if x2 > x1 {
                        x2.saturating_sub(1)
                    } else {
                        x2 + 1
                    },
                    y2,
                )
            };
            canvas.set_char(arrow_pos.0, arrow_pos.1, arrow);
        }
    }

    /// Get the appropriate corner character based on incoming and outgoing directions
    fn get_corner_char(
        &self,
        prev_x: usize,
        prev_y: usize,
        curr_x: usize,
        curr_y: usize,
        next_x: usize,
        next_y: usize,
    ) -> char {
        if self.style.is_ascii() {
            return '+';
        }

        // Determine incoming and outgoing directions
        let from_left = prev_x < curr_x;
        let from_right = prev_x > curr_x;
        let from_top = prev_y < curr_y;
        let from_bottom = prev_y > curr_y;

        let to_left = next_x < curr_x;
        let to_right = next_x > curr_x;
        let to_top = next_y < curr_y;
        let to_bottom = next_y > curr_y;

        match (
            from_left,
            from_right,
            from_top,
            from_bottom,
            to_left,
            to_right,
            to_top,
            to_bottom,
        ) {
            // Coming from left
            (true, _, _, _, _, _, true, _) => '┘', // left to up
            (true, _, _, _, _, _, _, true) => '┐', // left to down
            // Coming from right
            (_, true, _, _, _, _, true, _) => '└', // right to up
            (_, true, _, _, _, _, _, true) => '┌', // right to down
            // Coming from top
            (_, _, true, _, true, _, _, _) => '┘', // top to left
            (_, _, true, _, _, true, _, _) => '└', // top to right
            // Coming from bottom
            (_, _, _, true, true, _, _, _) => '┐', // bottom to left
            (_, _, _, true, _, true, _, _) => '┌', // bottom to right
            _ => '+',
        }
    }

    fn draw_edge_label(&self, canvas: &mut AsciiCanvas, waypoints: &[(usize, usize)], label: &str) {
        if waypoints.len() < 2 || label.is_empty() {
            return;
        }

        let (x1, y1) = waypoints[0];
        let (x2, y2) = waypoints[waypoints.len() - 1];

        if y1 == y2 {
            // Horizontal edge: place label above if possible, otherwise below
            let mid_x = (x1 + x2) / 2;
            let start_x = mid_x.saturating_sub(label.len() / 2);
            let label_y = if y1 > 0 { y1 - 1 } else { y1 + 1 };
            canvas.draw_text(start_x, label_y, label);
        } else if x1 == x2 {
            // Vertical edge: place label to the right of the line
            let mid_y = (y1 + y2) / 2;
            let label_x = x1 + 1;
            canvas.draw_text(label_x, mid_y, label);
        } else {
            // Orthogonal route (including splits): place label on the segment near target
            if y2 > y1 {
                // Going down: place label above the arrow, centered on the branch
                let label_y = y2.saturating_sub(2); // One row above arrow
                let label_x = x2.saturating_sub(label.len() / 2);
                canvas.draw_text(label_x, label_y, label);
            } else if y2 < y1 {
                // Going up: place label on the outside of the branch
                let label_y = y2 + 1; // Arrow row
                if x2 < x1 {
                    // Left branch: label to the left (with 1 char gap)
                    let label_x = x2.saturating_sub(label.len() + 1);
                    canvas.draw_text(label_x, label_y, label);
                } else {
                    // Right branch: label to the right
                    let label_x = x2 + 1;
                    canvas.draw_text(label_x, label_y, label);
                }
            } else if x2 > x1 {
                // Going right: place label above/below based on position
                if y2 < y1 {
                    // Upper branch: label above
                    let label_y = y2.saturating_sub(1);
                    let start_x = x2.saturating_sub(label.len());
                    canvas.draw_text(start_x, label_y, label);
                } else {
                    // Lower branch or straight: label below
                    let label_y = y2 + 1;
                    let start_x = x2.saturating_sub(label.len());
                    canvas.draw_text(start_x, label_y, label);
                }
            } else {
                // Going left: place label above/below based on position
                if y2 < y1 {
                    let label_y = y2.saturating_sub(1);
                    let start_x = x2 + 1;
                    canvas.draw_text(start_x, label_y, label);
                } else {
                    let label_y = y2 + 1;
                    let start_x = x2 + 1;
                    canvas.draw_text(start_x, label_y, label);
                }
            }
        }
    }

    fn draw_junction(
        &self,
        canvas: &mut AsciiCanvas,
        junction: (usize, usize),
        direction: crate::core::Direction,
        _group_size: usize,
    ) {
        let (jx, jy) = junction;

        // Draw the junction point
        // Junction receives line from source direction and splits perpendicular
        // TopDown: line comes from UP, splits LEFT/RIGHT → ┴
        // BottomUp: line comes from DOWN, splits LEFT/RIGHT → ┬
        // LeftRight: line comes from LEFT, splits UP/DOWN → ┤
        // RightLeft: line comes from RIGHT, splits UP/DOWN → ├
        let junction_char = match direction {
            crate::core::Direction::TopDown => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '┴'
                }
            }
            crate::core::Direction::BottomUp => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '┬'
                }
            }
            crate::core::Direction::LeftRight => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '┤'
                }
            }
            crate::core::Direction::RightLeft => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '├'
                }
            }
        };
        canvas.set_char(jx, jy, junction_char);
    }

    fn draw_split_edge(
        &self,
        canvas: &mut AsciiCanvas,
        from_center: (usize, usize),
        junction: (usize, usize),
        to_center: (usize, usize),
        edge_type: EdgeType,
        direction: crate::core::Direction,
    ) {
        let chars = EdgeChars::for_type(edge_type, self.style);
        if chars.is_invisible() {
            return;
        }

        let (fx, fy) = from_center;
        let (jx, jy) = junction;
        let (tx, ty) = to_center;
        let has_arrow = edge_type.has_arrow();

        match direction {
            crate::core::Direction::TopDown => {
                // Vertical from source to junction
                self.draw_vertical_line(canvas, fx, fy, jy, &chars);
                // Horizontal from junction toward target
                let corner_x = tx;
                if corner_x != jx {
                    self.draw_horizontal_line(
                        canvas,
                        jy,
                        jx.min(corner_x),
                        jx.max(corner_x),
                        &chars,
                    );
                }
                // Corner: line comes from junction (horizontal), goes down (vertical)
                // tx < jx: corner is left of junction, line comes from RIGHT, goes DOWN → ┌
                // tx > jx: corner is right of junction, line comes from LEFT, goes DOWN → ┐
                let corner = if self.style.is_ascii() {
                    '+'
                } else if tx < jx {
                    '┌'
                } else if tx > jx {
                    '┐'
                } else {
                    '│'
                };
                if corner_x != jx {
                    canvas.set_char(corner_x, jy, corner);
                }
                // Vertical down to target
                let end_y = if has_arrow { ty.saturating_sub(1) } else { ty };
                self.draw_vertical_line(canvas, corner_x, jy, end_y, &chars);
                if has_arrow {
                    canvas.set_char(corner_x, end_y, chars.arrow_down);
                }
            }
            crate::core::Direction::BottomUp => {
                // Similar but reversed
                self.draw_vertical_line(canvas, fx, jy, fy, &chars);
                let corner_x = tx;
                if corner_x != jx {
                    self.draw_horizontal_line(
                        canvas,
                        jy,
                        jx.min(corner_x),
                        jx.max(corner_x),
                        &chars,
                    );
                }
                // Corner: line comes from junction (horizontal), goes up (vertical)
                // tx < jx: corner is left of junction, line comes from RIGHT, goes UP → └
                // tx > jx: corner is right of junction, line comes from LEFT, goes UP → ┘
                let corner = if self.style.is_ascii() {
                    '+'
                } else if tx < jx {
                    '└'
                } else if tx > jx {
                    '┘'
                } else {
                    '│'
                };
                if corner_x != jx {
                    canvas.set_char(corner_x, jy, corner);
                }
                let end_y = if has_arrow { ty + 1 } else { ty };
                self.draw_vertical_line(canvas, corner_x, end_y, jy, &chars);
                if has_arrow {
                    canvas.set_char(corner_x, end_y, chars.arrow_up);
                }
            }
            crate::core::Direction::LeftRight => {
                // Horizontal from source to junction
                self.draw_horizontal_line(canvas, fy, fx, jx, &chars);
                // Vertical from junction toward target
                let corner_y = ty;
                if corner_y != jy {
                    self.draw_vertical_line(canvas, jx, jy.min(corner_y), jy.max(corner_y), &chars);
                }
                // Corner: line comes from junction (vertical), goes right (horizontal)
                // ty < jy: corner is above junction, line comes from BELOW, goes RIGHT → ┌
                // ty > jy: corner is below junction, line comes from ABOVE, goes RIGHT → └
                let corner = if self.style.is_ascii() {
                    '+'
                } else if ty < jy {
                    '┌'
                } else if ty > jy {
                    '└'
                } else {
                    '─'
                };
                if corner_y != jy {
                    canvas.set_char(jx, corner_y, corner);
                }
                // Horizontal to target
                let end_x = if has_arrow { tx.saturating_sub(1) } else { tx };
                self.draw_horizontal_line(canvas, corner_y, jx, end_x, &chars);
                if has_arrow {
                    canvas.set_char(end_x, corner_y, chars.arrow_right);
                }
            }
            crate::core::Direction::RightLeft => {
                // Similar but reversed
                self.draw_horizontal_line(canvas, fy, jx, fx, &chars);
                let corner_y = ty;
                if corner_y != jy {
                    self.draw_vertical_line(canvas, jx, jy.min(corner_y), jy.max(corner_y), &chars);
                }
                // Corner: line comes from junction (vertical), goes left (horizontal)
                // ty < jy: corner is above junction, line comes from BELOW, goes LEFT → ┐
                // ty > jy: corner is below junction, line comes from ABOVE, goes LEFT → ┘
                let corner = if self.style.is_ascii() {
                    '+'
                } else if ty < jy {
                    '┐'
                } else if ty > jy {
                    '┘'
                } else {
                    '─'
                };
                if corner_y != jy {
                    canvas.set_char(jx, corner_y, corner);
                }
                let end_x = if has_arrow { tx + 1 } else { tx };
                self.draw_horizontal_line(canvas, corner_y, end_x, jx, &chars);
                if has_arrow {
                    canvas.set_char(end_x, corner_y, chars.arrow_left);
                }
            }
        }
    }

    fn draw_merge_junction(
        &self,
        canvas: &mut AsciiCanvas,
        junction: (usize, usize),
        direction: crate::core::Direction,
    ) {
        let (jx, jy) = junction;

        // Merge junction: multiple lines come in, one goes out to target
        // TopDown: lines come from UP (multiple), merge goes DOWN → ┬
        // BottomUp: lines come from DOWN (multiple), merge goes UP → ┴
        // LeftRight: lines come from LEFT (multiple), merge goes RIGHT → ├
        // RightLeft: lines come from RIGHT (multiple), merge goes LEFT → ┤
        let junction_char = match direction {
            crate::core::Direction::TopDown => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '┬'
                }
            }
            crate::core::Direction::BottomUp => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '┴'
                }
            }
            crate::core::Direction::LeftRight => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '├'
                }
            }
            crate::core::Direction::RightLeft => {
                if self.style.is_ascii() {
                    '+'
                } else {
                    '┤'
                }
            }
        };
        canvas.set_char(jx, jy, junction_char);
    }

    fn draw_merge_edge(
        &self,
        canvas: &mut AsciiCanvas,
        from_center: (usize, usize),
        merge_junction: (usize, usize),
        to_center: (usize, usize),
        edge_type: EdgeType,
        direction: crate::core::Direction,
    ) {
        let chars = EdgeChars::for_type(edge_type, self.style);
        if chars.is_invisible() {
            return;
        }

        let (fx, fy) = from_center;
        let (mx, my) = merge_junction;
        let (_tx, _ty) = to_center; // Used only for context in some directions

        match direction {
            crate::core::Direction::TopDown => {
                // From source down to merge junction y-level, then horizontal to merge, then down to target
                // First: vertical from source to merge y
                let corner_x = fx;
                let corner_y = my;
                self.draw_vertical_line(canvas, corner_x, fy, corner_y, &chars);

                // Corner at (fx, my)
                let corner = if self.style.is_ascii() {
                    '+'
                } else if fx < mx {
                    '└' // coming from above, going right
                } else if fx > mx {
                    '┘' // coming from above, going left
                } else {
                    '│'
                };
                if corner_x != mx {
                    canvas.set_char(corner_x, corner_y, corner);
                }

                // Horizontal to merge junction
                if corner_x != mx {
                    self.draw_horizontal_line(
                        canvas,
                        corner_y,
                        corner_x.min(mx),
                        corner_x.max(mx),
                        &chars,
                    );
                }
            }
            crate::core::Direction::BottomUp => {
                // Similar but reversed
                let corner_x = fx;
                let corner_y = my;
                self.draw_vertical_line(canvas, corner_x, corner_y, fy, &chars);

                let corner = if self.style.is_ascii() {
                    '+'
                } else if fx < mx {
                    '┌' // coming from below, going right
                } else if fx > mx {
                    '┐' // coming from below, going left
                } else {
                    '│'
                };
                if corner_x != mx {
                    canvas.set_char(corner_x, corner_y, corner);
                    self.draw_horizontal_line(
                        canvas,
                        corner_y,
                        corner_x.min(mx),
                        corner_x.max(mx),
                        &chars,
                    );
                }
            }
            crate::core::Direction::LeftRight => {
                // From source right to merge junction x-level, then vertical to merge, then right to target
                let corner_x = mx;
                let corner_y = fy;

                // Horizontal from source to merge x
                self.draw_horizontal_line(canvas, corner_y, fx, corner_x, &chars);

                // Corner at (mx, fy)
                let corner = if self.style.is_ascii() {
                    '+'
                } else if fy < my {
                    '┐' // coming from left, going down
                } else if fy > my {
                    '┘' // coming from left, going up
                } else {
                    '─'
                };
                if corner_y != my {
                    canvas.set_char(corner_x, corner_y, corner);
                }

                // Vertical to merge junction
                if corner_y != my {
                    self.draw_vertical_line(
                        canvas,
                        corner_x,
                        corner_y.min(my),
                        corner_y.max(my),
                        &chars,
                    );
                }
            }
            crate::core::Direction::RightLeft => {
                // Similar but reversed
                let corner_x = mx;
                let corner_y = fy;

                self.draw_horizontal_line(canvas, corner_y, corner_x, fx, &chars);

                let corner = if self.style.is_ascii() {
                    '+'
                } else if fy < my {
                    '┌' // coming from right, going down
                } else if fy > my {
                    '└' // coming from right, going up
                } else {
                    '─'
                };
                if corner_y != my {
                    canvas.set_char(corner_x, corner_y, corner);
                    self.draw_vertical_line(
                        canvas,
                        corner_x,
                        corner_y.min(my),
                        corner_y.max(my),
                        &chars,
                    );
                }
            }
        }
    }

    fn draw_merge_to_target(
        &self,
        canvas: &mut AsciiCanvas,
        merge_junction: (usize, usize),
        to_center: (usize, usize),
        edge_type: EdgeType,
        direction: crate::core::Direction,
    ) {
        let chars = EdgeChars::for_type(edge_type, self.style);
        if chars.is_invisible() {
            return;
        }

        let (mx, my) = merge_junction;
        let (tx, ty) = to_center;
        let has_arrow = edge_type.has_arrow();

        match direction {
            crate::core::Direction::TopDown => {
                let end_y = if has_arrow { ty.saturating_sub(1) } else { ty };
                self.draw_vertical_line(canvas, mx, my, end_y, &chars);
                if has_arrow {
                    canvas.set_char(mx, end_y, chars.arrow_down);
                }
            }
            crate::core::Direction::BottomUp => {
                let end_y = if has_arrow { ty + 1 } else { ty };
                self.draw_vertical_line(canvas, mx, end_y, my, &chars);
                if has_arrow {
                    canvas.set_char(mx, end_y, chars.arrow_up);
                }
            }
            crate::core::Direction::LeftRight => {
                let end_x = if has_arrow { tx.saturating_sub(1) } else { tx };
                self.draw_horizontal_line(canvas, my, mx, end_x, &chars);
                if has_arrow {
                    canvas.set_char(end_x, my, chars.arrow_right);
                }
            }
            crate::core::Direction::RightLeft => {
                let end_x = if has_arrow { tx + 1 } else { tx };
                self.draw_horizontal_line(canvas, my, end_x, mx, &chars);
                if has_arrow {
                    canvas.set_char(end_x, my, chars.arrow_left);
                }
            }
        }
    }

    fn draw_horizontal_line(
        &self,
        canvas: &mut AsciiCanvas,
        y: usize,
        x1: usize,
        x2: usize,
        chars: &EdgeChars,
    ) {
        let (start, end) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
        let going_right = x2 > x1;
        let junction_t = if self.style.is_ascii() {
            '+'
        } else if going_right {
            '├'
        } else {
            '┤'
        };
        let junction_cross = if self.style.is_ascii() { '+' } else { '┼' };

        for x in start..=end {
            let existing = canvas.get_char(x, y);
            let is_start = x == start;
            let is_end = x == end;

            let new_char = match existing {
                ' ' => chars.horizontal,
                '│' | '┆' | '║' | '|' => {
                    // T-junction or crossing
                    if is_start || is_end {
                        junction_t
                    } else {
                        junction_cross // True crossing in the middle
                    }
                }
                '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '+' => {
                    existing
                } // Keep existing junctions
                _ => chars.horizontal,
            };
            canvas.set_char(x, y, new_char);
        }
    }

    fn draw_vertical_line(
        &self,
        canvas: &mut AsciiCanvas,
        x: usize,
        y1: usize,
        y2: usize,
        chars: &EdgeChars,
    ) {
        let (start, end) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
        let going_down = y2 > y1;
        let junction_t = if self.style.is_ascii() {
            '+'
        } else if going_down {
            '┬'
        } else {
            '┴'
        };
        let junction_cross = if self.style.is_ascii() { '+' } else { '┼' };

        for y in start..=end {
            let existing = canvas.get_char(x, y);
            let is_start = y == start;
            let is_end = y == end;

            let new_char = match existing {
                ' ' => chars.vertical,
                '─' | '┄' | '═' | '-' => {
                    // T-junction or crossing
                    if is_start || is_end {
                        junction_t
                    } else {
                        junction_cross // True crossing in the middle
                    }
                }
                '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '+' => {
                    existing
                } // Keep existing junctions
                _ => chars.vertical,
            };
            canvas.set_char(x, y, new_char);
        }
    }
}

/// Edge drawing characters
struct EdgeChars {
    horizontal: char,
    vertical: char,
    arrow_right: char,
    arrow_left: char,
    arrow_down: char,
    arrow_up: char,
    invisible: bool,
}

impl EdgeChars {
    fn for_type(edge_type: EdgeType, style: CharacterSet) -> Self {
        let ascii = matches!(style, CharacterSet::Ascii | CharacterSet::Compact);
        let dots = if ascii { '.' } else { '┄' };
        match edge_type {
            EdgeType::Arrow | EdgeType::Line | EdgeType::OpenArrow | EdgeType::CrossArrow => {
                if ascii {
                    Self {
                        horizontal: '-',
                        vertical: '|',
                        arrow_right: '>',
                        arrow_left: '<',
                        arrow_down: 'v',
                        arrow_up: '^',
                        invisible: false,
                    }
                } else {
                    Self {
                        horizontal: '─',
                        vertical: '│',
                        arrow_right: '▶',
                        arrow_left: '◀',
                        arrow_down: '▼',
                        arrow_up: '▲',
                        invisible: false,
                    }
                }
            }
            EdgeType::DottedArrow | EdgeType::DottedLine => {
                if ascii {
                    Self {
                        horizontal: dots,
                        vertical: ':',
                        arrow_right: '>',
                        arrow_left: '<',
                        arrow_down: 'v',
                        arrow_up: '^',
                        invisible: false,
                    }
                } else {
                    Self {
                        horizontal: '┄',
                        vertical: '┆',
                        arrow_right: '▷',
                        arrow_left: '◁',
                        arrow_down: '▽',
                        arrow_up: '△',
                        invisible: false,
                    }
                }
            }
            EdgeType::ThickArrow | EdgeType::ThickLine => {
                if ascii {
                    Self {
                        horizontal: '=',
                        vertical: '|',
                        arrow_right: '>',
                        arrow_left: '<',
                        arrow_down: 'v',
                        arrow_up: '^',
                        invisible: false,
                    }
                } else {
                    Self {
                        horizontal: '═',
                        vertical: '║',
                        arrow_right: '▶',
                        arrow_left: '◀',
                        arrow_down: '▼',
                        arrow_up: '▲',
                        invisible: false,
                    }
                }
            }
            EdgeType::Invisible => Self {
                horizontal: ' ',
                vertical: ' ',
                arrow_right: ' ',
                arrow_left: ' ',
                arrow_down: ' ',
                arrow_up: ' ',
                invisible: true,
            },
        }
    }

    fn is_invisible(&self) -> bool {
        self.invisible
    }
}

impl Default for FlowchartRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer<FlowchartDatabase> for FlowchartRenderer {
    type Output = String;

    fn render(&self, database: &FlowchartDatabase) -> Result<Self::Output> {
        let render_span = span!(
            Level::INFO,
            "render_flowchart",
            style = ?self.style,
            node_count = database.node_count(),
            edge_count = database.edge_count()
        );
        let _enter = render_span.enter(); // Enter span to track duration

        trace!("Starting flowchart rendering");

        // First, compute the layout
        let layout_algo = FlowchartLayoutAlgorithm::new();
        let layout = layout_algo.layout(database)?;

        if layout.nodes.is_empty() {
            debug!("Empty layout, returning empty string");
            return Ok(String::new());
        }

        // Create canvas
        let canvas_span = span!(
            Level::DEBUG,
            "create_canvas",
            width = layout.width,
            height = layout.height
        );
        let _canvas_enter = canvas_span.enter();
        let mut canvas = AsciiCanvas::new(layout.width, layout.height);
        debug!("Created ASCII canvas");
        drop(_canvas_enter);

        // Draw subgraphs first (background layer)
        let subgraph_span = span!(
            Level::DEBUG,
            "draw_subgraphs",
            subgraph_count = layout.subgraphs.len()
        );
        let _subgraph_enter = subgraph_span.enter();
        for subgraph in &layout.subgraphs {
            trace!(
                subgraph_id = %subgraph.id,
                subgraph_title = %subgraph.title,
                x = subgraph.x,
                y = subgraph.y,
                width = subgraph.width,
                height = subgraph.height,
                "Drawing subgraph"
            );
            self.draw_subgraph(&mut canvas, subgraph);
        }
        debug!(subgraph_count = layout.subgraphs.len(), "Drew subgraphs");
        drop(_subgraph_enter);

        // Draw edges first (so nodes overlay them)
        let edge_span = span!(Level::DEBUG, "draw_edges", edge_count = layout.edges.len());
        let _edge_enter = edge_span.enter();
        let mut edges_drawn = 0;

        // Track which junctions we've drawn (split junctions)
        let mut drawn_split_junctions: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::new();
        // Track which merge junctions we've drawn and drawn the final segment for
        let mut drawn_merge_junctions: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::new();

        // Collect labels to draw after all edges (so labels don't interfere with edge drawing)
        let mut labels_to_draw: Vec<(Vec<(usize, usize)>, String)> = Vec::new();

        // First pass: draw all edge lines
        for edge in &layout.edges {
            let edge_data = database
                .edges()
                .find(|e| e.from == edge.from_id && e.to == edge.to_id);
            let edge_type = edge_data.map(|e| e.edge_type).unwrap_or(EdgeType::Arrow);
            let edge_label = edge_data.and_then(|e| e.label.as_deref());
            trace!(
                edge_from = %edge.from_id,
                edge_to = %edge.to_id,
                edge_type = ?edge_type,
                edge_label = ?edge_label,
                waypoint_count = edge.waypoints.len(),
                has_junction = edge.junction.is_some(),
                has_merge_junction = edge.merge_junction.is_some(),
                "Drawing edge"
            );

            let from_node = layout.nodes.iter().find(|n| n.id == edge.from_id);
            let to_node = layout.nodes.iter().find(|n| n.id == edge.to_id);

            // Compute edge exit/entry points based on direction
            let (from_center, to_center) = if let (Some(from), Some(to)) = (from_node, to_node) {
                let fc = match database.direction() {
                    crate::core::Direction::TopDown => {
                        (from.x + from.width / 2, from.y + from.height)
                    }
                    crate::core::Direction::BottomUp => (from.x + from.width / 2, from.y),
                    crate::core::Direction::LeftRight => {
                        (from.x + from.width, from.y + from.height / 2)
                    }
                    crate::core::Direction::RightLeft => (from.x, from.y + from.height / 2),
                };
                let tc = match database.direction() {
                    crate::core::Direction::TopDown => (to.x + to.width / 2, to.y),
                    crate::core::Direction::BottomUp => (to.x + to.width / 2, to.y + to.height),
                    crate::core::Direction::LeftRight => (to.x, to.y + to.height / 2),
                    crate::core::Direction::RightLeft => (to.x + to.width, to.y + to.height / 2),
                };
                (Some(fc), Some(tc))
            } else {
                (None, None)
            };

            // Handle split junction (edges from same source)
            if let Some(junction) = edge.junction {
                // Draw junction if not already drawn
                if !drawn_split_junctions.contains(&junction) {
                    self.draw_junction(
                        &mut canvas,
                        junction,
                        database.direction(),
                        edge.group_size.unwrap_or(1),
                    );
                    drawn_split_junctions.insert(junction);
                }

                // Draw split edge through junction
                if let (Some(fc), Some(tc)) = (from_center, to_center) {
                    // If this edge also has a merge junction, draw split to merge, not to target
                    if let Some(merge_junction) = edge.merge_junction {
                        // Split edge goes: source -> split junction -> ... -> merge junction
                        // We'll handle the merge part separately
                        self.draw_split_edge(
                            &mut canvas,
                            fc,
                            junction,
                            merge_junction,
                            edge_type,
                            database.direction(),
                        );
                    } else {
                        self.draw_split_edge(
                            &mut canvas,
                            fc,
                            junction,
                            tc,
                            edge_type,
                            database.direction(),
                        );
                    }
                }
            }
            // Handle merge junction (edges to same target)
            // Skip merge junction handling for back-edges (they have special routing)
            else if let Some(merge_junction) = edge.merge_junction {
                if edge.waypoints.len() > 2 {
                    // Back-edge with special routing - use the waypoints directly
                    self.draw_edge(&mut canvas, &edge.waypoints, edge_type);
                } else if let (Some(fc), Some(tc)) = (from_center, to_center) {
                    // Draw edge from source to merge junction
                    self.draw_merge_edge(
                        &mut canvas,
                        fc,
                        merge_junction,
                        tc,
                        edge_type,
                        database.direction(),
                    );

                    // Draw merge junction and final segment only once
                    if !drawn_merge_junctions.contains(&merge_junction) {
                        self.draw_merge_junction(&mut canvas, merge_junction, database.direction());
                        self.draw_merge_to_target(
                            &mut canvas,
                            merge_junction,
                            tc,
                            edge_type,
                            database.direction(),
                        );
                        drawn_merge_junctions.insert(merge_junction);
                    }
                }
            } else {
                // Regular edge (no split, no merge)
                self.draw_edge(&mut canvas, &edge.waypoints, edge_type);
            }

            // Collect label for later drawing
            if let Some(label) = edge_label {
                labels_to_draw.push((edge.waypoints.clone(), label.to_string()));
            }
            edges_drawn += 1;
        }

        // Second pass: draw all labels (after edge lines, so they overlay correctly)
        for (waypoints, label) in &labels_to_draw {
            self.draw_edge_label(&mut canvas, waypoints, label);
        }
        debug!(edges_drawn, "Drew edges");
        drop(_edge_enter);

        // Draw nodes
        let node_span = span!(Level::DEBUG, "draw_nodes", node_count = layout.nodes.len());
        let _node_enter = node_span.enter();
        let mut nodes_drawn = 0;
        for node in &layout.nodes {
            if let Some(node_data) = database.get_node(&node.id) {
                trace!(
                    node_id = %node.id,
                    node_shape = ?node_data.shape,
                    node_label = %node_data.label,
                    node_x = node.x,
                    node_y = node.y,
                    node_width = node.width,
                    node_height = node.height,
                    "Drawing node"
                );
                self.draw_node(&mut canvas, node, node_data.shape, &node_data.label);
                nodes_drawn += 1;
            }
        }
        debug!(nodes_drawn, "Drew nodes");
        drop(_node_enter);

        // Redraw subgraph titles last to fix overlap with nodes/edges
        for subgraph in &layout.subgraphs {
            self.redraw_subgraph_title(&mut canvas, subgraph);
        }

        let output = canvas.to_string();
        info!(
            output_len = output.len(),
            canvas_width = layout.width,
            canvas_height = layout.height,
            "Rendering completed"
        );

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "ascii"
    }

    fn version(&self) -> &'static str {
        "0.2.0"
    }

    fn format(&self) -> &'static str {
        "ascii"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CharacterSet, Direction};

    #[test]
    fn test_basic_rendering() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(!output.is_empty());
        assert!(output.contains("Start"));
        assert!(output.contains("End"));
    }

    #[test]
    fn test_empty_database() {
        let db = FlowchartDatabase::new();
        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.is_empty());
    }

    #[test]
    fn test_diamond_shape() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);
        db.add_shaped_node("A", "Yes?", crate::core::NodeShape::Diamond)
            .unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.contains("Yes?"));
        // Diamond should have corner characters (◆ for Box style, < > for Tall)
        assert!(
            output.contains('◆') || output.contains('<') || output.contains('>'),
            "Expected diamond corner chars in: {}",
            output
        );
    }

    #[test]
    fn test_renderer_properties() {
        let renderer = FlowchartRenderer::new();
        assert_eq!(renderer.name(), "ascii");
        assert_eq!(renderer.format(), "ascii");
    }

    #[test]
    fn test_edge_labels_are_drawn() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "End").unwrap();
        db.add_labeled_edge("A", "B", EdgeType::Arrow, "yes")
            .unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.contains("yes"));
    }

    #[test]
    fn test_ascii_style_uses_ascii_chars() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();

        let renderer = FlowchartRenderer::with_style(CharacterSet::Ascii);
        let output = renderer.render(&db).unwrap();

        assert!(output.contains('+'));
        assert!(!output.contains('┌'));
        assert!(output.contains('>') || output.contains('-'));
    }

    #[test]
    fn test_split_junction_lr() {
        // A -> B, A -> C (split from A)
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("A", "C").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have split junction character ┤
        assert!(
            output.contains('┤'),
            "Expected split junction ┤ in output:\n{}",
            output
        );
    }

    #[test]
    fn test_merge_junction_lr() {
        // B -> D, C -> D (merge into D)
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("B", "D").unwrap();
        db.add_simple_edge("C", "D").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have merge junction character ├
        assert!(
            output.contains('├'),
            "Expected merge junction ├ in output:\n{}",
            output
        );
    }

    #[test]
    fn test_diamond_pattern_lr() {
        // Diamond: A -> B, A -> C, B -> D, C -> D
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();
        db.add_simple_edge("C", "D").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have both split junction ┤ (from A) and merge junction ├ (into D)
        assert!(
            output.contains('┤'),
            "Expected split junction ┤ in output:\n{}",
            output
        );
        assert!(
            output.contains('├'),
            "Expected merge junction ├ in output:\n{}",
            output
        );
    }

    #[test]
    fn test_merge_junction_td() {
        // B -> D, C -> D (merge into D) in top-down direction
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("B", "D").unwrap();
        db.add_simple_edge("C", "D").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have merge junction character ┬ (for TD)
        assert!(
            output.contains('┬'),
            "Expected merge junction ┬ in output:\n{}",
            output
        );
    }

    #[test]
    fn test_three_way_merge_lr() {
        // B -> E, C -> E, D -> E (three edges merging)
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_node("E", "E").unwrap();
        db.add_simple_edge("B", "E").unwrap();
        db.add_simple_edge("C", "E").unwrap();
        db.add_simple_edge("D", "E").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have merge junction character ├
        assert!(
            output.contains('├'),
            "Expected merge junction ├ in output:\n{}",
            output
        );
        // All three sources should be present
        assert!(output.contains("B"));
        assert!(output.contains("C"));
        assert!(output.contains("D"));
        assert!(output.contains("E"));
    }

    #[test]
    fn test_three_way_split_lr() {
        // A -> B, A -> C, A -> D (three edges splitting)
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("A", "D").unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have split junction character ┤
        assert!(
            output.contains('┤'),
            "Expected split junction ┤ in output:\n{}",
            output
        );
        // All nodes should be present
        assert!(output.contains("A"));
        assert!(output.contains("B"));
        assert!(output.contains("C"));
        assert!(output.contains("D"));
    }
}
