//! Git graph database implementation
//!
//! Stores commits, branches, and their relationships.

use crate::core::{Database, Direction, EdgeData, NodeData, NodeShape};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// Git graph database
pub struct GitGraphDatabase {
    nodes: HashMap<String, NodeData>,
    edges: Vec<EdgeData>,
    direction: Direction,
}

impl GitGraphDatabase {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            direction: Direction::TopDown, // Default to top-down, but can be changed
        }
    }

    pub fn with_direction(direction: Direction) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            direction,
        }
    }

    pub fn add_commit(
        &mut self,
        id: impl Into<String>,
        message: Option<impl Into<String>>,
    ) -> Result<()> {
        let id = id.into();
        let label = message.map(|m| m.into()).unwrap_or_else(|| id.clone());

        if self.nodes.contains_key(&id) {
            return Ok(()); // Already exists
        }

        let node = NodeData::with_shape(&id, &label, NodeShape::Circle);
        self.nodes.insert(id.clone(), node);
        debug!(commit_id = %id, "Added commit to database");
        Ok(())
    }

    pub fn add_parent_edge(
        &mut self,
        child: impl Into<String>,
        parent: impl Into<String>,
    ) -> Result<()> {
        let child = child.into();
        let parent = parent.into();

        // Ensure both commits exist
        if !self.nodes.contains_key(&child) {
            self.add_commit(&child, None::<String>)?;
        }
        if !self.nodes.contains_key(&parent) {
            self.add_commit(&parent, None::<String>)?;
        }

        let edge = EdgeData::new(&child, &parent);
        self.edges.push(edge);
        Ok(())
    }
}

impl Default for GitGraphDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for GitGraphDatabase {
    type Node = NodeData;
    type Edge = EdgeData;

    fn add_node(&mut self, node: NodeData) -> Result<()> {
        let id = node.id.clone();
        self.nodes.insert(id, node);
        Ok(())
    }

    fn add_edge(&mut self, edge: EdgeData) -> Result<()> {
        // Ensure both nodes exist
        if !self.nodes.contains_key(&edge.from) {
            return Err(anyhow::anyhow!("Node '{}' not found", edge.from));
        }
        if !self.nodes.contains_key(&edge.to) {
            return Err(anyhow::anyhow!("Node '{}' not found", edge.to));
        }

        self.edges.push(edge);
        Ok(())
    }

    fn get_node(&self, id: &str) -> Option<&NodeData> {
        self.nodes.get(id)
    }

    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn edge_count(&self) -> usize {
        self.edges.len()
    }

    fn nodes(&self) -> impl Iterator<Item = &NodeData> {
        self.nodes.values()
    }

    fn edges(&self) -> impl Iterator<Item = &EdgeData> {
        self.edges.iter()
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }
}

impl GitGraphDatabase {
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }

    pub fn source_nodes(&self) -> Vec<&str> {
        let targets: HashSet<&str> = self.edges.iter().map(|e| e.to.as_str()).collect();
        self.nodes
            .keys()
            .filter(|id| !targets.contains(id.as_str()))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sink_nodes(&self) -> Vec<&str> {
        let sources: HashSet<&str> = self.edges.iter().map(|e| e.from.as_str()).collect();
        self.nodes
            .keys()
            .filter(|id| !sources.contains(id.as_str()))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn predecessors(&self, id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.to == id)
            .map(|e| e.from.as_str())
            .collect()
    }

    pub fn successors(&self, id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .map(|e| e.to.as_str())
            .collect()
    }

    pub fn in_degree(&self, id: &str) -> usize {
        self.edges.iter().filter(|e| e.to == id).count()
    }

    pub fn out_degree(&self, id: &str) -> usize {
        self.edges.iter().filter(|e| e.from == id).count()
    }

    pub fn topological_sort(&self) -> Vec<&str> {
        // For git graphs, we want commits in chronological order (oldest first)
        // This is a simple topological sort using Kahn's algorithm
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

        // Initialize in-degrees and adjacency list
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id.as_str(), 0);
            adjacency.insert(node_id.as_str(), Vec::new());
        }

        for edge in &self.edges {
            *in_degree.get_mut(edge.to.as_str()).unwrap() += 1;
            adjacency
                .get_mut(edge.from.as_str())
                .unwrap()
                .push(edge.to.as_str());
        }

        // Find all nodes with in-degree 0 (sorted for deterministic output)
        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();
        queue.sort(); // Ensure deterministic order

        let mut result = Vec::new();

        // Process nodes (use pop to maintain sorted order - process last alphabetically first)
        while let Some(node_id) = queue.pop() {
            result.push(node_id);

            // Reduce in-degree of neighbors
            if let Some(neighbors) = adjacency.get(node_id) {
                for &neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        // Insert in sorted position for deterministic order
                        let pos = queue.binary_search(&neighbor).unwrap_or_else(|p| p);
                        queue.insert(pos, neighbor);
                    }
                }
            }
        }

        // Add any remaining nodes (cycles) in sorted order for determinism
        let mut remaining: Vec<&str> = self
            .nodes
            .keys()
            .map(|s| s.as_str())
            .filter(|id| !result.contains(id))
            .collect();
        remaining.sort();
        result.extend(remaining);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_commit() {
        let mut db = GitGraphDatabase::new();
        db.add_commit("c1", Some("Initial commit")).unwrap();
        assert_eq!(db.node_count(), 1);
        assert!(db.has_node("c1"));
    }

    #[test]
    fn test_add_parent_edge() {
        let mut db = GitGraphDatabase::new();
        db.add_commit("c1", None::<String>).unwrap();
        db.add_commit("c2", None::<String>).unwrap();
        db.add_parent_edge("c2", "c1").unwrap();
        assert_eq!(db.edge_count(), 1);
    }
}
