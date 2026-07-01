//! Core diagram trait for all diagram types
//!
//! This trait defines the interface that all diagram implementations must follow.
//! It's inspired by mermaid.js's plugin system but adapted for Rust with SOLID principles.

use super::{Database, Detector, Parser, Renderer};
use std::sync::Arc;

/// Core trait for diagram types
///
/// This trait represents a complete diagram type with its associated components.
/// Each diagram type (flowchart, sequence, class diagram, etc.) should implement this trait.
///
/// # Example
/// ```
/// use figurehead::core::Diagram;
/// use figurehead::plugins::flowchart::FlowchartDiagram;
///
/// let _diagram = FlowchartDiagram;
/// ```
pub trait Diagram: Send + Sync {
    /// The specific database type for this diagram
    type Database: Database + Send + Sync;

    /// The parser type for this diagram
    type Parser: Parser<Self::Database> + Send + Sync;

    /// The renderer type for this diagram
    type Renderer: Renderer<Self::Database> + Send + Sync;

    /// Get the detector for this diagram type
    fn detector() -> Arc<dyn Detector>;

    /// Create a new parser instance
    fn create_parser() -> Self::Parser;

    /// Create a new database instance
    fn create_database() -> Self::Database;

    /// Create a new renderer instance
    fn create_renderer() -> Self::Renderer;

    /// Get the name of this diagram type
    fn name() -> &'static str;

    /// Get the version of this diagram type
    fn version() -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::flowchart::*;

    #[test]
    fn test_diagram_trait_exists() {
        // This test ensures we have the core Diagram trait
        let _diagram = FlowchartDiagram;
        assert_eq!(FlowchartDiagram::name(), "flowchart");
        assert_eq!(FlowchartDiagram::version(), "0.1.0");
    }

    #[test]
    fn test_flowchart_diagram_implementation() {
        // This test ensures Flowchart implements the Diagram trait
        assert_eq!(FlowchartDiagram::name(), "flowchart");
        assert_eq!(FlowchartDiagram::version(), "0.1.0");

        // Test that we can create components
        let _parser = FlowchartDiagram::create_parser();
        let _database = FlowchartDiagram::create_database();
        let _renderer = FlowchartDiagram::create_renderer();
        let _detector = FlowchartDiagram::detector();
    }
}
