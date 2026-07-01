//! Core renderer trait for diagram output
//!
//! This trait defines the interface for rendering diagram data
//! into various output formats (ASCII, SVG, etc.).

use anyhow::Result;

use super::Database;

/// Core trait for diagram renderers
///
/// This trait represents the rendering layer that converts diagram data
/// into visual output. Each diagram type can have multiple renderers.
///
/// # Example
/// ```
/// use figurehead::core::{Renderer, Database};
/// use figurehead::plugins::flowchart::{FlowchartDatabase, FlowchartRenderer};
///
/// let db = FlowchartDatabase::new();
/// let renderer = FlowchartRenderer::new();
/// let output = renderer.render(&db).unwrap();
/// ```
pub trait Renderer<D: Database>: Send + Sync {
    /// The output type of this renderer
    type Output;

    /// Render the diagram database into the output format
    fn render(&self, database: &D) -> Result<Self::Output>;

    /// Get the name of this renderer
    fn name(&self) -> &'static str;

    /// Get the version of this renderer
    fn version(&self) -> &'static str;

    /// Get the supported output format
    fn format(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::flowchart::*;

    #[test]
    fn test_diagram_renderer_trait_exists() {
        // This test ensures we have the DiagramRenderer trait
        let renderer = FlowchartRenderer::new();
        assert_eq!(renderer.name(), "ascii");
        assert_eq!(renderer.version(), "0.2.0");
        assert_eq!(renderer.format(), "ascii");
    }

    #[test]
    fn test_basic_rendering() {
        let renderer = FlowchartRenderer::new();
        let mut database = FlowchartDatabase::new();

        database.add_simple_node("A", "Node A").unwrap();
        database.add_simple_node("B", "Node B").unwrap();
        database.add_simple_edge("A", "B").unwrap();

        let output = renderer.render(&database).unwrap();
        // The new renderer produces ASCII art with box characters
        assert!(output.contains("Node A"));
        assert!(output.contains("Node B"));
    }
}
