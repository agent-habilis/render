//! ASCII rendering implementation for git graphs
//!
//! Converts positioned commits into ASCII diagrams.

use anyhow::Result;
use tracing::{debug, info, span, trace, Level};

use super::layout::{GitGraphLayoutAlgorithm, PositionedCommit};
use super::GitGraphDatabase;
use crate::core::{AsciiCanvas, CharacterSet, Database, LayoutAlgorithm, Renderer};

/// Git graph ASCII renderer
pub struct GitGraphRenderer {
    style: CharacterSet,
}

impl GitGraphRenderer {
    pub fn new() -> Self {
        Self {
            style: CharacterSet::default(),
        }
    }

    pub fn with_style(style: CharacterSet) -> Self {
        Self { style }
    }

    fn draw_commit(&self, canvas: &mut AsciiCanvas, commit: &PositionedCommit, label: &str) {
        let x = commit.x + commit.width / 2;
        let y = commit.y + commit.height / 2;

        // Draw commit as circle (using * or ○)
        let commit_char = if self.style.is_ascii() { '*' } else { '○' };
        canvas.set_char(x, y, commit_char);

        // Draw label below commit
        let label_x = x.saturating_sub(label.len() / 2);
        canvas.draw_text(label_x.max(0), commit.y + commit.height + 1, label);
    }

    fn draw_edge(&self, canvas: &mut AsciiCanvas, waypoints: &[(usize, usize)]) {
        if waypoints.len() < 2 {
            return;
        }

        let (x1, y1) = waypoints[0];
        let (x2, y2) = waypoints[waypoints.len() - 1];

        let line_char = if self.style.is_ascii() { '|' } else { '│' };
        let h_line_char = if self.style.is_ascii() { '-' } else { '─' };

        // Draw based on orientation
        if x1 == x2 {
            // Pure vertical line
            let (start, end) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
            for y in start..=end {
                if canvas.get_char(x1, y) == ' ' {
                    canvas.set_char(x1, y, line_char);
                }
            }
        } else if y1 == y2 {
            // Pure horizontal line
            let (start, end) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
            for x in start..=end {
                if canvas.get_char(x, y1) == ' ' {
                    canvas.set_char(x, y1, h_line_char);
                }
            }
        } else {
            // L-shaped path: draw both segments
            // Vertical segment
            let (v_start, v_end) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
            for y in v_start..=v_end {
                if canvas.get_char(x1, y) == ' ' {
                    canvas.set_char(x1, y, line_char);
                }
            }

            // Horizontal segment
            let (h_start, h_end) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
            let mid_y = if y1 == y2 { y1 } else { y1.min(y2) };
            for x in h_start..=h_end {
                let existing = canvas.get_char(x, mid_y);
                if existing == ' ' || existing == line_char {
                    canvas.set_char(x, mid_y, h_line_char);
                }
            }

            // Corner
            let corner = if self.style.is_ascii() {
                '+'
            } else if y1 < y2 {
                '└'
            } else {
                '┌'
            };
            canvas.set_char(x2, mid_y, corner);
        }
    }
}

impl Default for GitGraphRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer<GitGraphDatabase> for GitGraphRenderer {
    type Output = String;

    fn render(&self, database: &GitGraphDatabase) -> Result<Self::Output> {
        let render_span = span!(
            Level::INFO,
            "render_gitgraph",
            style = ?self.style,
            commit_count = database.node_count(),
            edge_count = database.edge_count()
        );
        let _enter = render_span.enter();

        trace!("Starting git graph rendering");

        // Compute layout
        let layout_algo = GitGraphLayoutAlgorithm::new();
        let layout = layout_algo.layout(database)?;

        if layout.commits.is_empty() {
            debug!("Empty layout, returning empty string");
            return Ok(String::new());
        }

        // Create canvas
        let mut canvas = AsciiCanvas::new(layout.width, layout.height);

        // Draw edges first (so commits overlay them)
        for edge in &layout.edges {
            self.draw_edge(&mut canvas, &edge.waypoints);
        }

        // Draw commits
        for commit in &layout.commits {
            if let Some(node_data) = database.get_node(&commit.id) {
                self.draw_commit(&mut canvas, commit, &node_data.label);
            }
        }

        let output = canvas.to_string();
        info!(
            output_len = output.len(),
            canvas_width = layout.width,
            canvas_height = layout.height,
            "Git graph rendering completed"
        );

        Ok(output)
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

    #[test]
    fn test_basic_rendering() {
        let mut db = GitGraphDatabase::new();
        db.add_commit("c1", Some("Initial")).unwrap();
        db.add_commit("c2", Some("Feature")).unwrap();
        db.add_parent_edge("c2", "c1").unwrap();

        let renderer = GitGraphRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(!output.is_empty());
        assert!(output.contains("Initial") || output.contains("Feature"));
    }
}
