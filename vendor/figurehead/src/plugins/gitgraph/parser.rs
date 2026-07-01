//! Git graph parser implementation
//!
//! Parses git graph syntax using the syntax parser abstraction.

use super::syntax_parser::GitGraphSyntaxParser;
use super::GitGraphDatabase;
use crate::core::{Database, EdgeData, NodeData, NodeShape, Parser, SyntaxParser};
use anyhow::Result;
use tracing::{debug, info, span, trace, Level};

/// Git graph parser implementation
pub struct GitGraphParser {
    syntax_parser: GitGraphSyntaxParser,
}

impl GitGraphParser {
    pub fn new() -> Self {
        Self {
            syntax_parser: GitGraphSyntaxParser::new(),
        }
    }
}

impl Default for GitGraphParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser<GitGraphDatabase> for GitGraphParser {
    fn parse(&self, input: &str, database: &mut GitGraphDatabase) -> Result<()> {
        let parse_span = span!(Level::INFO, "parse_gitgraph", input_len = input.len());
        let _enter = parse_span.enter();

        trace!("Starting git graph parsing");

        // Check for direction specification: gitGraph TD or gitGraph LR
        let lines: Vec<&str> = input.lines().map(|l| l.trim()).collect();
        for line in &lines {
            let line_lower = line.to_lowercase();
            if line_lower.starts_with("gitgraph") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(direction) = parts[1].parse::<crate::core::Direction>() {
                        database.set_direction(direction);
                        debug!(direction = ?direction, "Parsed git graph direction");
                    }
                }
            }
        }

        // Parse syntax into AST
        let syntax_nodes = self.syntax_parser.parse(input)?;
        debug!(
            syntax_node_count = syntax_nodes.len(),
            "Parsed syntax nodes"
        );

        let mut node_count = 0;
        let mut edge_count = 0;

        // Convert syntax nodes to database operations
        for syntax_node in syntax_nodes {
            match syntax_node {
                crate::core::SyntaxNode::Node {
                    id,
                    label,
                    metadata: _metadata,
                } => {
                    // Create commit node
                    let shape = NodeShape::Circle; // Commits are circles

                    let node = NodeData::with_shape(&id, label.as_deref().unwrap_or(&id), shape);
                    database.add_node(node)?;
                    node_count += 1;
                }
                crate::core::SyntaxNode::Edge {
                    from,
                    to,
                    label,
                    metadata: _metadata,
                } => {
                    // Create parent edge (in git, edges go from child to parent)
                    let edge = if let Some(label) = label {
                        EdgeData::with_label(&from, &to, crate::core::EdgeType::Arrow, label)
                    } else {
                        EdgeData::new(&from, &to)
                    };
                    database.add_edge(edge)?;
                    edge_count += 1;
                }
                crate::core::SyntaxNode::Group { .. } => {
                    // Groups not yet supported for git graphs
                    debug!("Skipping group node (not yet supported)");
                }
            }
        }

        info!(
            node_count,
            edge_count, "Git graph parsing completed successfully"
        );

        Ok(())
    }

    fn name(&self) -> &'static str {
        "gitgraph"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        self.syntax_parser.can_parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_linear_graph() {
        let parser = GitGraphParser::new();
        let mut database = GitGraphDatabase::new();

        let input = "gitGraph\n   commit\n   commit\n   commit";
        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 3);
        assert_eq!(database.edge_count(), 2);
    }

    #[test]
    fn test_parse_with_branches() {
        let parser = GitGraphParser::new();
        let mut database = GitGraphDatabase::new();

        let input = r#"gitGraph
   commit
   branch develop
   checkout develop
   commit"#;
        parser.parse(input, &mut database).unwrap();
        // Should have 2 commits + 1 branch node = 3 nodes
        assert_eq!(database.node_count(), 3);
    }
}
