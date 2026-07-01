//! Git graph layout implementation
//!
//! Arranges commits in a chronological graph layout.

use anyhow::Result;
use std::collections::HashMap;
use tracing::{info, span, trace, Level};
use unicode_width::UnicodeWidthStr;

use super::GitGraphDatabase;
use crate::core::{Database, Direction, LayoutAlgorithm};

/// Position data for a laid out commit
#[derive(Debug, Clone)]
pub struct PositionedCommit {
    pub id: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Position data for a laid out edge
#[derive(Debug, Clone)]
pub struct PositionedEdge {
    pub from_id: String,
    pub to_id: String,
    pub waypoints: Vec<(usize, usize)>,
}

/// Layout output containing positioned elements
#[derive(Debug)]
pub struct GitGraphLayoutResult {
    pub commits: Vec<PositionedCommit>,
    pub edges: Vec<PositionedEdge>,
    pub width: usize,
    pub height: usize,
}

/// Git graph layout algorithm
pub struct GitGraphLayoutAlgorithm;

impl GitGraphLayoutAlgorithm {
    pub fn new() -> Self {
        Self
    }

    fn calculate_commit_size(&self, label: &str) -> (usize, usize) {
        let label_width = UnicodeWidthStr::width(label);
        let width = (label_width + 4).max(8); // Padding for circle
        let height = 3; // Standard height for commit circle
        (width, height)
    }
}

impl Default for GitGraphLayoutAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutAlgorithm<GitGraphDatabase> for GitGraphLayoutAlgorithm {
    type Output = GitGraphLayoutResult;

    fn layout(&self, database: &GitGraphDatabase) -> Result<Self::Output> {
        let layout_span = span!(
            Level::INFO,
            "layout_gitgraph",
            commit_count = database.node_count(),
            edge_count = database.edge_count()
        );
        let _enter = layout_span.enter();

        trace!("Starting git graph layout");

        let nodes: Vec<_> = database.nodes().collect();
        if nodes.is_empty() {
            return Ok(GitGraphLayoutResult {
                commits: Vec::new(),
                edges: Vec::new(),
                width: 0,
                height: 0,
            });
        }

        // Calculate commit sizes
        let mut commit_sizes: HashMap<&str, (usize, usize)> = HashMap::new();
        for node in &nodes {
            let size = self.calculate_commit_size(&node.label);
            commit_sizes.insert(&node.id, size);
        }

        // Topological sort to get chronological order
        let sorted = database.topological_sort();
        let direction = database.direction();

        // Assign positions based on direction
        let mut positioned_commits = Vec::new();
        let mut max_width = 0;
        let mut max_height = 0;

        let node_sep = 4; // Spacing between commits
        let padding = 2;

        match direction {
            Direction::TopDown | Direction::BottomUp => {
                // Vertical layout: commits arranged top to bottom
                let mut y = padding;
                let center_x = padding + 10usize; // Center line for commits

                let commit_ids: Vec<&str> = if direction.is_reversed() {
                    sorted.iter().rev().copied().collect()
                } else {
                    sorted.to_vec()
                };

                for commit_id in commit_ids {
                    if let Some(_node) = database.get_node(commit_id) {
                        let (width, height) = commit_sizes[commit_id];
                        let x = center_x.saturating_sub(width / 2);

                        positioned_commits.push(PositionedCommit {
                            id: commit_id.to_string(),
                            x,
                            y,
                            width,
                            height,
                        });

                        max_width = max_width.max(x + width + padding);
                        y += height + node_sep;
                    }
                }
                max_height = y + padding;
            }
            Direction::LeftRight | Direction::RightLeft => {
                // Horizontal layout: commits arranged left to right
                let mut x = padding;
                let center_y = padding + 2usize; // Center line for commits

                let commit_ids: Vec<&str> = if direction.is_reversed() {
                    sorted.iter().rev().copied().collect()
                } else {
                    sorted.to_vec()
                };

                for commit_id in commit_ids {
                    if let Some(_node) = database.get_node(commit_id) {
                        let (width, height) = commit_sizes[commit_id];
                        let y = center_y.saturating_sub(height / 2);

                        positioned_commits.push(PositionedCommit {
                            id: commit_id.to_string(),
                            x,
                            y,
                            width,
                            height,
                        });

                        max_height = max_height.max(y + height + padding);
                        x += width + node_sep;
                    }
                }
                max_width = x + padding;
            }
        }

        // Route edges (parent relationships)
        let mut positioned_edges = Vec::new();
        let commit_positions: HashMap<&str, &PositionedCommit> = positioned_commits
            .iter()
            .map(|c| (c.id.as_str(), c))
            .collect();

        for edge in database.edges() {
            if let (Some(from), Some(to)) = (
                commit_positions.get(edge.from.as_str()),
                commit_positions.get(edge.to.as_str()),
            ) {
                // Calculate edge waypoints based on direction
                let (exit_x, exit_y, entry_x, entry_y) = match direction {
                    Direction::TopDown => {
                        // Child (bottom) to parent (top)
                        (
                            from.x + from.width / 2,
                            from.y,
                            to.x + to.width / 2,
                            to.y + to.height,
                        )
                    }
                    Direction::BottomUp => {
                        // Reversed: parent (top) to child (bottom)
                        (
                            from.x + from.width / 2,
                            from.y + from.height,
                            to.x + to.width / 2,
                            to.y,
                        )
                    }
                    Direction::LeftRight => {
                        // Child (left) to parent (right)
                        (
                            from.x + from.width,
                            from.y + from.height / 2,
                            to.x,
                            to.y + to.height / 2,
                        )
                    }
                    Direction::RightLeft => {
                        // Reversed: parent (right) to child (left)
                        (
                            from.x,
                            from.y + from.height / 2,
                            to.x + to.width,
                            to.y + to.height / 2,
                        )
                    }
                };

                positioned_edges.push(PositionedEdge {
                    from_id: edge.from.clone(),
                    to_id: edge.to.clone(),
                    waypoints: vec![(exit_x, exit_y), (entry_x, entry_y)],
                });
            }
        }

        info!(
            commit_count = positioned_commits.len(),
            edge_count = positioned_edges.len(),
            width = max_width,
            height = max_height,
            "Git graph layout completed"
        );

        Ok(GitGraphLayoutResult {
            commits: positioned_commits,
            edges: positioned_edges,
            width: max_width,
            height: max_height,
        })
    }

    fn name(&self) -> &'static str {
        "gitgraph"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn direction(&self) -> &'static str {
        "top-down"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_layout() {
        let mut db = GitGraphDatabase::new();
        db.add_commit("c1", Some("Initial")).unwrap();
        db.add_commit("c2", Some("Feature")).unwrap();
        db.add_parent_edge("c2", "c1").unwrap();

        let layout = GitGraphLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.commits.len(), 2);
        assert_eq!(result.edges.len(), 1);
        assert!(result.width > 0);
        assert!(result.height > 0);
    }
}
