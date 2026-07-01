//! Core database trait for diagram data storage
//!
//! This trait defines the interface for storing and managing diagram data.
//! Each diagram type implements this with its own node and edge data types.

use anyhow::Result;

/// Core trait for diagram databases
///
/// This trait represents the data storage layer for diagram information.
/// Each diagram type has its own database implementation that stores
/// nodes, edges, and other diagram-specific data.
///
/// The associated types allow each diagram type to define its own
/// node and edge structures with type-specific metadata.
pub trait Database: Send + Sync {
    /// The node data type for this database
    type Node: Clone + Send + Sync;

    /// The edge data type for this database
    type Edge: Clone + Send + Sync;

    /// Add a node to the database
    fn add_node(&mut self, node: Self::Node) -> Result<()>;

    /// Add an edge to the database
    fn add_edge(&mut self, edge: Self::Edge) -> Result<()>;

    /// Get a node by ID
    fn get_node(&self, id: &str) -> Option<&Self::Node>;

    /// Iterate over all nodes
    fn nodes(&self) -> impl Iterator<Item = &Self::Node>;

    /// Iterate over all edges
    fn edges(&self) -> impl Iterator<Item = &Self::Edge>;

    /// Clear all data from the database
    fn clear(&mut self);

    /// Get the number of nodes
    fn node_count(&self) -> usize;

    /// Get the number of edges
    fn edge_count(&self) -> usize;
}
