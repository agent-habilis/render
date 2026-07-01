//! State diagram ASCII renderer
//!
//! Renders state diagrams as ASCII art.

use super::database::{StateDatabase, START_TERMINAL};
use super::layout::{PositionedTransition, StateLayoutAlgorithm, StateLayoutResult};
use crate::core::{AsciiCanvas, CharacterSet, NodeShape, Renderer};
use anyhow::Result;
use std::collections::HashMap;

/// Box drawing characters
struct BoxChars {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
}

impl BoxChars {
    fn unicode() -> Self {
        Self {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
        }
    }

    fn ascii() -> Self {
        Self {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
        }
    }
}

/// State diagram renderer
pub struct StateRenderer {
    style: CharacterSet,
}

impl StateRenderer {
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

    fn box_chars(&self) -> BoxChars {
        if self.is_unicode() {
            BoxChars::unicode()
        } else {
            BoxChars::ascii()
        }
    }

    /// Draw a terminal state (start/end circle)
    fn draw_terminal(
        &self,
        canvas: &mut AsciiCanvas,
        x: usize,
        y: usize,
        width: usize,
        is_start: bool,
    ) {
        let center_x = x + width / 2;

        if self.is_unicode() {
            if is_start {
                canvas.draw_text_centered(center_x, y + 1, "(●)");
            } else {
                canvas.draw_text_centered(center_x, y + 1, "(○)");
            }
        } else if is_start {
            canvas.draw_text_centered(center_x, y + 1, "(*)");
        } else {
            canvas.draw_text_centered(center_x, y + 1, "(o)");
        }
    }

    /// Draw a state box
    fn draw_state_box(
        &self,
        canvas: &mut AsciiCanvas,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        label: &str,
    ) {
        let chars = self.box_chars();

        // Top border
        canvas.set_char(x, y, chars.top_left);
        for i in 1..width - 1 {
            canvas.set_char(x + i, y, chars.horizontal);
        }
        canvas.set_char(x + width - 1, y, chars.top_right);

        // Sides
        for row in 1..height - 1 {
            canvas.set_char(x, y + row, chars.vertical);
            canvas.set_char(x + width - 1, y + row, chars.vertical);
        }

        // Bottom border
        canvas.set_char(x, y + height - 1, chars.bottom_left);
        for i in 1..width - 1 {
            canvas.set_char(x + i, y + height - 1, chars.horizontal);
        }
        canvas.set_char(x + width - 1, y + height - 1, chars.bottom_right);

        // Center label
        let center_x = x + width / 2;
        let center_y = y + height / 2;
        canvas.draw_text_centered(center_x, center_y, label);
    }

    /// Draw a single edge between two points with optional label
    fn draw_single_edge(
        &self,
        canvas: &mut AsciiCanvas,
        from_x: usize,
        from_y: usize,
        to_x: usize,
        to_y: usize,
        label: Option<&str>,
    ) {
        if from_y >= to_y {
            return;
        }

        let arrow_down = if self.is_unicode() { '▼' } else { 'v' };
        let v_line = if self.is_unicode() { '│' } else { '|' };
        let h_line = if self.is_unicode() { '─' } else { '-' };

        if from_x == to_x {
            // Straight vertical line
            for y in from_y..to_y {
                canvas.set_char(from_x, y, v_line);
            }
            canvas.set_char(from_x, to_y, arrow_down);

            // Draw label to the right of the line
            if let Some(lbl) = label {
                if !lbl.is_empty() {
                    let label_y = from_y + (to_y - from_y) / 2;
                    canvas.draw_text(from_x + 2, label_y, lbl);
                }
            }
        } else {
            // Orthogonal routing: vertical down, then horizontal, then vertical to target
            let mid_y = from_y + (to_y - from_y) / 2;

            // First vertical segment
            for y in from_y..mid_y {
                canvas.set_char(from_x, y, v_line);
            }

            // Corner at source x
            let corner1 = if self.is_unicode() {
                if to_x > from_x {
                    '└'
                } else {
                    '┘'
                }
            } else {
                '+'
            };
            canvas.set_char(from_x, mid_y, corner1);

            // Horizontal segment
            let (h_start, h_end) = if to_x > from_x {
                (from_x + 1, to_x)
            } else {
                (to_x + 1, from_x)
            };
            for x in h_start..h_end {
                canvas.set_char(x, mid_y, h_line);
            }

            // Corner at target x
            let corner2 = if self.is_unicode() {
                if to_x > from_x {
                    '┐'
                } else {
                    '┌'
                }
            } else {
                '+'
            };
            canvas.set_char(to_x, mid_y, corner2);

            // Second vertical segment
            for y in (mid_y + 1)..to_y {
                canvas.set_char(to_x, y, v_line);
            }
            canvas.set_char(to_x, to_y, arrow_down);

            // Draw label on horizontal segment
            if let Some(lbl) = label {
                if !lbl.is_empty() {
                    let label_x = (from_x + to_x) / 2;
                    let label_start = label_x.saturating_sub(lbl.chars().count() / 2);
                    canvas.draw_text(label_start, mid_y.saturating_sub(1), lbl);
                }
            }
        }
    }

    /// Draw split edges (one source to multiple targets)
    fn draw_split_edges(
        &self,
        canvas: &mut AsciiCanvas,
        from_x: usize,
        from_y: usize,
        targets: &[(usize, usize, Option<&str>)], // (to_x, to_y, label)
    ) {
        if targets.is_empty() {
            return;
        }

        let arrow_down = if self.is_unicode() { '▼' } else { 'v' };
        let v_line = if self.is_unicode() { '│' } else { '|' };
        let h_line = if self.is_unicode() { '─' } else { '-' };

        // Find the min to_y to determine junction row
        let min_to_y = targets.iter().map(|(_, y, _)| *y).min().unwrap_or(from_y);
        let junction_y = from_y + (min_to_y - from_y) / 2;

        // Find the span of target x positions
        let min_x = targets.iter().map(|(x, _, _)| *x).min().unwrap_or(from_x);
        let max_x = targets.iter().map(|(x, _, _)| *x).max().unwrap_or(from_x);

        // Draw vertical line from source down to junction
        for y in from_y..junction_y {
            canvas.set_char(from_x, y, v_line);
        }

        // Draw horizontal bar across all targets
        for x in min_x..=max_x {
            canvas.set_char(x, junction_y, h_line);
        }

        // Draw junction character where source line meets the bar
        // Use ┴ since line comes from ABOVE and goes LEFT/RIGHT (no DOWN at this point)
        let junction_char = if self.is_unicode() {
            if from_x <= min_x {
                '└' // Source at left edge
            } else if from_x >= max_x {
                '┘' // Source at right edge
            } else {
                '┴' // Source in middle - connects UP, LEFT, RIGHT
            }
        } else {
            '+'
        };
        canvas.set_char(from_x, junction_y, junction_char);

        // Draw corners and vertical lines to each target
        for (to_x, to_y, label) in targets {
            // Corner at target x on junction row
            let corner = if self.is_unicode() {
                if *to_x == min_x {
                    '┌'
                } else if *to_x == max_x {
                    '┐'
                } else {
                    '┬'
                }
            } else {
                '+'
            };
            canvas.set_char(*to_x, junction_y, corner);

            // Vertical line from junction to target
            for y in (junction_y + 1)..*to_y {
                canvas.set_char(*to_x, y, v_line);
            }
            canvas.set_char(*to_x, *to_y, arrow_down);

            // Draw label above the corner
            if let Some(lbl) = label {
                if !lbl.is_empty() {
                    let label_start = to_x.saturating_sub(lbl.chars().count() / 2);
                    canvas.draw_text(label_start, junction_y.saturating_sub(1), lbl);
                }
            }
        }
    }

    /// Draw merge edges (multiple sources to one target)
    fn draw_merge_edges(
        &self,
        canvas: &mut AsciiCanvas,
        sources: &[(usize, usize)], // (from_x, from_y)
        to_x: usize,
        to_y: usize,
    ) {
        if sources.is_empty() {
            return;
        }

        let arrow_down = if self.is_unicode() { '▼' } else { 'v' };
        let v_line = if self.is_unicode() { '│' } else { '|' };
        let h_line = if self.is_unicode() { '─' } else { '-' };

        // Find the max from_y to determine junction row
        let max_from_y = sources.iter().map(|(_, y)| *y).max().unwrap_or(0);
        let junction_y = max_from_y + (to_y - max_from_y) / 2;

        // Find the span of source x positions
        let min_x = sources.iter().map(|(x, _)| *x).min().unwrap_or(to_x);
        let max_x = sources.iter().map(|(x, _)| *x).max().unwrap_or(to_x);

        // Draw vertical lines from each source to junction row
        for (from_x, from_y) in sources {
            for y in *from_y..junction_y {
                canvas.set_char(*from_x, y, v_line);
            }
            // Corner at source x on junction row
            let corner = if self.is_unicode() {
                if *from_x == min_x {
                    '└'
                } else if *from_x == max_x {
                    '┘'
                } else {
                    '┴'
                }
            } else {
                '+'
            };
            canvas.set_char(*from_x, junction_y, corner);
        }

        // Draw horizontal bar
        for x in min_x..=max_x {
            // Don't overwrite corners
            let current = canvas
                .grid
                .get(junction_y)
                .and_then(|row| row.get(x))
                .copied();
            if current == Some(' ') || current == Some(h_line) {
                canvas.set_char(x, junction_y, h_line);
            }
        }

        // Draw junction character where target line meets the bar
        // Use ┬ since line goes DOWN from the bar
        let junction_char = if self.is_unicode() {
            if to_x <= min_x {
                '└' // Target at left edge
            } else if to_x >= max_x {
                '┘' // Target at right edge
            } else {
                '┬' // Target in middle - line goes DOWN
            }
        } else {
            '+'
        };
        canvas.set_char(to_x, junction_y, junction_char);

        // Draw vertical line from junction to target
        for y in (junction_y + 1)..to_y {
            canvas.set_char(to_x, y, v_line);
        }
        canvas.set_char(to_x, to_y, arrow_down);
    }

    /// Render the layout result
    fn render_layout(&self, layout: &StateLayoutResult) -> String {
        if layout.states.is_empty() {
            return String::new();
        }

        // Calculate canvas size with extra space for arrows
        let extra_height = layout.transitions.len() * 2;
        let width = layout.width + 20; // Extra space for labels
        let height = layout.height + extra_height + 2;

        let mut canvas = AsciiCanvas::new(width, height);

        // Draw states
        for state in &layout.states {
            match state.shape {
                NodeShape::Terminal => {
                    // Use ID to determine if start or end terminal
                    let is_start = state.id == START_TERMINAL;
                    self.draw_terminal(&mut canvas, state.x, state.y, state.width, is_start);
                }
                _ => {
                    self.draw_state_box(
                        &mut canvas,
                        state.x,
                        state.y,
                        state.width,
                        state.height,
                        &state.label,
                    );
                }
            }
        }

        // Group transitions by source for split detection
        let mut by_source: HashMap<String, Vec<&PositionedTransition>> = HashMap::new();
        for trans in &layout.transitions {
            by_source
                .entry(trans.from_id.clone())
                .or_default()
                .push(trans);
        }

        // Group transitions by target for merge detection
        let mut by_target: HashMap<String, Vec<&PositionedTransition>> = HashMap::new();
        for trans in &layout.transitions {
            by_target
                .entry(trans.to_id.clone())
                .or_default()
                .push(trans);
        }

        // Track which transitions we've already drawn
        let mut drawn: std::collections::HashSet<(&str, &str)> = std::collections::HashSet::new();

        // Draw splits (one source to multiple targets)
        for transitions in by_source.values() {
            if transitions.len() > 1 {
                let first = transitions[0];
                let targets: Vec<(usize, usize, Option<&str>)> = transitions
                    .iter()
                    .map(|t| (t.to_x, t.to_y.saturating_sub(1), t.label.as_deref()))
                    .collect();
                self.draw_split_edges(&mut canvas, first.from_x, first.from_y, &targets);
                for t in transitions {
                    drawn.insert((&t.from_id, &t.to_id));
                }
            }
        }

        // Draw merges (multiple sources to one target)
        for transitions in by_target.values() {
            if transitions.len() > 1 {
                // Check if any of these are already drawn (part of a split)
                let undrawn: Vec<_> = transitions
                    .iter()
                    .filter(|t| !drawn.contains(&(t.from_id.as_str(), t.to_id.as_str())))
                    .collect();
                if undrawn.len() > 1 {
                    let first = undrawn[0];
                    let sources: Vec<(usize, usize)> =
                        undrawn.iter().map(|t| (t.from_x, t.from_y)).collect();
                    self.draw_merge_edges(
                        &mut canvas,
                        &sources,
                        first.to_x,
                        first.to_y.saturating_sub(1),
                    );
                    for t in undrawn {
                        drawn.insert((&t.from_id, &t.to_id));
                    }
                }
            }
        }

        // Draw remaining single edges
        for trans in &layout.transitions {
            if !drawn.contains(&(trans.from_id.as_str(), trans.to_id.as_str())) {
                self.draw_single_edge(
                    &mut canvas,
                    trans.from_x,
                    trans.from_y,
                    trans.to_x,
                    trans.to_y.saturating_sub(1),
                    trans.label.as_deref(),
                );
            }
        }

        canvas.to_string()
    }

    /// Render the database to ASCII
    pub fn render(&self, database: &StateDatabase) -> Result<String> {
        let layout_algo = StateLayoutAlgorithm::new();
        let layout = layout_algo.layout(database)?;

        Ok(self.render_layout(&layout))
    }
}

impl Default for StateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer<StateDatabase> for StateRenderer {
    type Output = String;

    fn render(&self, database: &StateDatabase) -> Result<Self::Output> {
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
    use super::*;
    use crate::core::{EdgeData, NodeData};

    #[test]
    fn test_render_empty() {
        let db = StateDatabase::new();
        let renderer = StateRenderer::new();
        let output = renderer.render(&db).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_render_single_state() {
        let mut db = StateDatabase::new();
        db.add_state(NodeData::new("Idle", "Idle")).unwrap();

        let renderer = StateRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.contains("Idle"));
        assert!(output.contains('┌') || output.contains('+'));
    }

    #[test]
    fn test_render_with_terminal() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("[*]", "Idle")).unwrap();

        let renderer = StateRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Should have terminal marker and state
        assert!(output.contains("●") || output.contains("*"));
        assert!(output.contains("Idle"));
    }

    #[test]
    fn test_render_ascii_mode() {
        let mut db = StateDatabase::new();
        db.add_state(NodeData::new("Test", "Test")).unwrap();

        let renderer = StateRenderer::with_style(CharacterSet::Ascii);
        let output = renderer.render(&db).unwrap();

        // Should use ASCII characters
        assert!(output.contains('+'));
        assert!(output.contains('-'));
    }

    #[test]
    fn test_render_branching() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("[*]", "Idle")).unwrap();
        db.add_transition(EdgeData::new("Idle", "Processing"))
            .unwrap();
        db.add_transition(EdgeData::new("Processing", "Success"))
            .unwrap();
        db.add_transition(EdgeData::new("Processing", "Failed"))
            .unwrap();
        db.add_transition(EdgeData::new("Success", "[*]")).unwrap();
        db.add_transition(EdgeData::new("Failed", "[*]")).unwrap();

        let renderer = StateRenderer::new();
        let output = renderer.render(&db).unwrap();

        // Check that the start terminal has leading spaces (is centered)
        let first_line = output.lines().next().unwrap();
        assert!(
            first_line.starts_with("  ") || first_line.trim().is_empty(),
            "First line should have leading spaces or be empty, got: '{}'",
            first_line
        );
    }
}
