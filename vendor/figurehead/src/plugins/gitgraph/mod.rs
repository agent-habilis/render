//! Git graph diagram plugin
//!
//! Implements git commit graph visualization with ASCII art syntax.
//!
//! Syntax examples:
//! - Linear: `*--*--*`
//! - With labels: `*[commit msg]--*[another]--*`
//! - Branches: `*--*--*` with branch labels
//! - Merges: `*--*--*` showing merge commits

mod database;
mod detector;
mod layout;
mod parser;
mod renderer;
mod syntax_parser;

pub use database::GitGraphDatabase;
pub use detector::GitGraphDetector;
pub use layout::{GitGraphLayoutAlgorithm, GitGraphLayoutResult};
pub use parser::GitGraphParser;
pub use renderer::GitGraphRenderer;
pub use syntax_parser::GitGraphSyntaxParser;

use crate::core::{Detector, Diagram};
use std::sync::Arc;

/// Git graph diagram implementation
pub struct GitGraphDiagram;

impl Diagram for GitGraphDiagram {
    type Database = GitGraphDatabase;
    type Parser = GitGraphParser;
    type Renderer = GitGraphRenderer;

    fn detector() -> Arc<dyn Detector> {
        Arc::new(GitGraphDetector::new())
    }

    fn create_parser() -> Self::Parser {
        GitGraphParser::new()
    }

    fn create_database() -> Self::Database {
        GitGraphDatabase::new()
    }

    fn create_renderer() -> Self::Renderer {
        GitGraphRenderer::new()
    }

    fn name() -> &'static str {
        "gitgraph"
    }

    fn version() -> &'static str {
        "0.1.0"
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::core::{Database, Parser, Renderer};

    #[test]
    fn test_full_pipeline() {
        let detector = GitGraphDiagram::detector();
        let parser = GitGraphDiagram::create_parser();
        let mut database = GitGraphDiagram::create_database();
        let renderer = GitGraphDiagram::create_renderer();

        let input = "gitGraph\n   commit\n   commit\n   commit";
        assert!(detector.detect(input));

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 3);
        assert_eq!(database.edge_count(), 2);

        let output = renderer.render(&database).unwrap();
        assert!(!output.is_empty());
    }
}
