//! Sequence diagram ASCII renderer
//!
//! Renders sequence diagrams as ASCII art.

use anyhow::Result;

use super::database::{ArrowHead, ArrowType, LineStyle, SequenceDatabase};
use super::layout::SequenceLayoutAlgorithm;
use crate::core::{AsciiCanvas, CharacterSet};

/// Sequence diagram renderer
pub struct SequenceRenderer {
    style: CharacterSet,
}

impl SequenceRenderer {
    pub fn new() -> Self {
        Self {
            style: CharacterSet::default(),
        }
    }

    pub fn with_style(style: CharacterSet) -> Self {
        Self { style }
    }

    fn is_unicode(&self) -> bool {
        !self.style.is_ascii()
    }

    /// Draw a horizontal line with style options
    fn draw_styled_horizontal(
        &self,
        canvas: &mut AsciiCanvas,
        x1: usize,
        x2: usize,
        y: usize,
        solid: bool,
    ) {
        let (start, end) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
        let line_char = if solid {
            if self.is_unicode() {
                '─'
            } else {
                '-'
            }
        } else if self.is_unicode() {
            '╌'
        } else {
            '-'
        };
        for x in start..=end {
            // LOCAL PATCH (ah-r): a horizontal message crossing a lifeline drew
            // `─` over the `│`, breaking the vertical line. Draw a 4-way junction
            // at the intersection instead so the two lines meet seamlessly.
            let ch = match canvas.get_char(x, y) {
                '│' | '|' if self.is_unicode() => '┼',
                '│' | '|' => '+',
                _ => line_char,
            };
            canvas.set_char(x, y, ch);
        }
    }

    /// Draw a vertical lifeline
    fn draw_lifeline(&self, canvas: &mut AsciiCanvas, x: usize, y1: usize, y2: usize) {
        let (start, end) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
        let line_char = if self.is_unicode() { '│' } else { '|' };
        for y in start..=end {
            // Don't overwrite existing non-space characters (like arrows)
            if canvas.get_char(x, y) == ' ' {
                canvas.set_char(x, y, line_char);
            }
        }
    }

    /// Draw a participant header box
    fn draw_participant(
        &self,
        canvas: &mut AsciiCanvas,
        x: usize,
        y: usize,
        label: &str,
        width: usize,
    ) {
        let unicode = self.is_unicode();

        // Draw box around label
        let left = x.saturating_sub(width / 2);
        let right = left + width - 1;

        if unicode {
            // Top border
            canvas.set_char(left, y, '┌');
            for i in (left + 1)..right {
                canvas.set_char(i, y, '─');
            }
            canvas.set_char(right, y, '┐');

            // Sides and label
            canvas.set_char(left, y + 1, '│');
            canvas.set_char(right, y + 1, '│');

            // Bottom border
            canvas.set_char(left, y + 2, '└');
            for i in (left + 1)..right {
                canvas.set_char(i, y + 2, '─');
            }
            canvas.set_char(right, y + 2, '┘');
        } else {
            // ASCII box
            canvas.set_char(left, y, '+');
            for i in (left + 1)..right {
                canvas.set_char(i, y, '-');
            }
            canvas.set_char(right, y, '+');

            canvas.set_char(left, y + 1, '|');
            canvas.set_char(right, y + 1, '|');

            canvas.set_char(left, y + 2, '+');
            for i in (left + 1)..right {
                canvas.set_char(i, y + 2, '-');
            }
            canvas.set_char(right, y + 2, '+');
        }

        // Center the label
        canvas.draw_text_centered(x, y + 1, label);
    }

    /// Draw a message arrow with label
    fn draw_message(
        &self,
        canvas: &mut AsciiCanvas,
        from_x: usize,
        to_x: usize,
        y: usize,
        label: &str,
        arrow: &ArrowType,
    ) {
        let unicode = self.is_unicode();
        let going_right = to_x > from_x;
        let solid = arrow.line == LineStyle::Solid;

        // Determine arrow head character
        let has_head = arrow.head != ArrowHead::None;
        let arrow_char = match arrow.head {
            ArrowHead::Arrow if unicode => {
                if going_right {
                    '▶'
                } else {
                    '◀'
                }
            }
            ArrowHead::Arrow => {
                if going_right {
                    '>'
                } else {
                    '<'
                }
            }
            ArrowHead::Open => {
                if going_right {
                    ')'
                } else {
                    '('
                }
            }
            ArrowHead::None => ' ',
        };

        // LOCAL PATCH (ah-r): a headed arrow stops one column short of the
        // receiver's lifeline so the `│` stays unbroken (`──▶│`). A headless line
        // runs to the lifeline column, where draw_styled_horizontal turns the
        // crossing into `┼`.
        let (line_start, line_end, head_x) = if going_right {
            if has_head {
                (from_x + 1, to_x.saturating_sub(2), to_x.saturating_sub(1))
            } else {
                (from_x + 1, to_x, to_x)
            }
        } else if has_head {
            (to_x + 2, from_x.saturating_sub(1), to_x + 1)
        } else {
            (to_x, from_x.saturating_sub(1), to_x)
        };

        if line_start < line_end {
            self.draw_styled_horizontal(canvas, line_start, line_end, y, solid);
        }

        // LOCAL PATCH (ah-r): connect the line to the sender's lifeline with a
        // 3-way T-junction (├ leaving right, ┤ leaving left) instead of leaving a
        // detached `│` beside the line. Self-messages (from == to) span no line.
        if from_x != to_x {
            let junction = match (going_right, unicode) {
                (true, true) => '├',
                (false, true) => '┤',
                (_, false) => '+',
            };
            canvas.set_char(from_x, y, junction);
        }

        // Draw arrow head (one column short of the receiver, see above).
        if has_head {
            canvas.set_char(head_x, y, arrow_char);
        }

        // LOCAL PATCH (ah-r): pad the label with a space each side so the text
        // doesn't abut the arrow line (`──── label ────` instead of
        // `────label────`).
        if !label.is_empty() {
            let center_x = (from_x + to_x) / 2;
            canvas.draw_text_centered(center_x, y, &format!(" {label} "));
        }
    }

    /// Render the database to ASCII
    pub fn render(&self, database: &SequenceDatabase) -> Result<String> {
        let layout_algo = SequenceLayoutAlgorithm::new();
        let layout = layout_algo.layout(database)?;

        if layout.participants.is_empty() {
            return Ok(String::new());
        }

        let mut canvas = AsciiCanvas::new(layout.width, layout.height);

        // Draw participant headers
        for participant in &layout.participants {
            self.draw_participant(
                &mut canvas,
                participant.x,
                0,
                &participant.label,
                participant.width,
            );
        }

        // Draw lifelines
        for participant in &layout.participants {
            self.draw_lifeline(
                &mut canvas,
                participant.x,
                layout.lifeline_start_y,
                layout.height - 1,
            );
        }

        // Draw messages
        for msg in &layout.messages {
            self.draw_message(
                &mut canvas,
                msg.from_x,
                msg.to_x,
                msg.y,
                &msg.label,
                &msg.arrow,
            );
        }

        Ok(canvas.to_string())
    }
}

impl Default for SequenceRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::core::Renderer<SequenceDatabase> for SequenceRenderer {
    type Output = String;

    fn render(&self, database: &SequenceDatabase) -> Result<Self::Output> {
        self.render(database)
    }

    fn name(&self) -> &'static str {
        "ascii"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn format(&self) -> &'static str {
        "ascii"
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::{ArrowType, Message, Participant};
    use super::*;

    #[test]
    fn test_render_single_message() {
        let mut db = SequenceDatabase::new();
        db.add_message(Message::new("Alice", "Bob", "Hello"))
            .unwrap();

        let renderer = SequenceRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(!output.is_empty());
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
        assert!(output.contains("Hello"));
    }

    #[test]
    fn test_render_multiple_messages() {
        let mut db = SequenceDatabase::new();
        db.add_message(Message::new("Alice", "Bob", "Hello"))
            .unwrap();
        db.add_message(Message::new("Bob", "Alice", "Hi")).unwrap();

        let renderer = SequenceRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.contains("Hello"));
        assert!(output.contains("Hi"));
    }

    #[test]
    fn test_render_with_alias() {
        let mut db = SequenceDatabase::new();
        db.add_participant(Participant::with_label("A", "Alice"))
            .unwrap();
        db.add_participant(Participant::with_label("B", "Bob"))
            .unwrap();
        db.add_message(Message::new("A", "B", "Hi")).unwrap();

        let renderer = SequenceRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should show labels, not ids
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }

    #[test]
    fn test_render_empty_database() {
        let db = SequenceDatabase::new();
        let renderer = SequenceRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.is_empty());
    }

    #[test]
    fn test_render_dotted_arrow() {
        let mut db = SequenceDatabase::new();
        let msg = Message::new("Alice", "Bob", "Response").with_arrow(ArrowType::dotted_arrow());
        db.add_message(msg).unwrap();

        let renderer = SequenceRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should contain dotted line character
        assert!(output.contains('╌') || output.contains('-'));
    }
}
