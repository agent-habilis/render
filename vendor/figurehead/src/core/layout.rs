//! Core layout trait for diagram positioning
//!
//! This trait defines the interface for arranging diagram elements
//! in a coordinate system, inspired by Dagre layout algorithms.

use anyhow::Result;

use super::Database;

/// Core trait for layout algorithms
///
/// This trait represents the layout layer that arranges diagram elements
/// in a coordinate system. Each diagram type can have different layout
/// strategies optimized for its specific needs.
///
/// # Example
/// ```
/// use figurehead::core::{LayoutAlgorithm, Database};
/// use figurehead::plugins::flowchart::{FlowchartDatabase, FlowchartLayoutAlgorithm};
///
/// let db = FlowchartDatabase::new();
/// let layout = FlowchartLayoutAlgorithm::new();
/// let positioned_data = layout.layout(&db).unwrap();
/// ```
pub trait LayoutAlgorithm<D: Database>: Send + Sync {
    /// The output type of this layout algorithm
    type Output;

    /// Arrange elements in the database using this layout algorithm
    fn layout(&self, database: &D) -> Result<Self::Output>;

    /// Get the name of this layout algorithm
    fn name(&self) -> &'static str;

    /// Get the version of this layout algorithm
    fn version(&self) -> &'static str;

    /// Get the layout direction (LR, TB, etc.)
    fn direction(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::flowchart::*;

    #[test]
    fn test_layout_algorithm_trait_exists() {
        // This test ensures we have the LayoutAlgorithm trait
        let layout = FlowchartLayoutAlgorithm::new();
        assert_eq!(layout.name(), "sugiyama");
        assert_eq!(layout.version(), "0.2.0");
        assert_eq!(layout.direction(), "configurable");
    }

    #[test]
    fn test_basic_layout() {
        let layout = FlowchartLayoutAlgorithm::new();
        let mut database = FlowchartDatabase::new();

        database.add_simple_node("A", "Node A").unwrap();
        database.add_simple_node("B", "Node B").unwrap();

        let output = layout.layout(&database).unwrap();
        assert_eq!(output.nodes.len(), 2);
        assert!(output.width > 0);
        assert!(output.height > 0);
    }
}
