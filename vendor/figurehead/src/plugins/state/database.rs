//! State diagram database implementation
//!
//! Stores states and transitions for state diagrams using core types.

use crate::core::{Database, EdgeData, NodeData, NodeShape};
use anyhow::Result;

/// Internal ID for start terminal
pub const START_TERMINAL: &str = "[*]_start";
/// Internal ID for end terminal
pub const END_TERMINAL: &str = "[*]_end";

/// State diagram database using core NodeData and EdgeData
#[derive(Debug, Default)]
pub struct StateDatabase {
    states: Vec<NodeData>,
    transitions: Vec<EdgeData>,
    has_start: bool,
    has_end: bool,
}

impl StateDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a state
    pub fn add_state(&mut self, state: NodeData) -> Result<()> {
        // Don't add duplicates
        if !self.states.iter().any(|s| s.id == state.id) {
            self.states.push(state);
        }
        Ok(())
    }

    /// Ensure a state exists (creates implicit state if needed)
    fn ensure_state_internal(&mut self, id: &str) -> Result<()> {
        if !self.states.iter().any(|s| s.id == id) {
            let shape = if id == START_TERMINAL || id == END_TERMINAL {
                NodeShape::Terminal
            } else {
                NodeShape::Rectangle
            };
            self.states.push(NodeData::with_shape(id, id, shape));
        }
        Ok(())
    }

    /// Add a transition, handling [*] as start or end terminal
    pub fn add_transition(&mut self, transition: EdgeData) -> Result<()> {
        // Handle [*] specially - first as source = start, as target = end
        let from = if transition.from == "[*]" {
            self.has_start = true;
            START_TERMINAL.to_string()
        } else {
            transition.from.clone()
        };

        let to = if transition.to == "[*]" {
            self.has_end = true;
            END_TERMINAL.to_string()
        } else {
            transition.to.clone()
        };

        // Ensure states exist
        self.ensure_state_internal(&from)?;
        self.ensure_state_internal(&to)?;

        // Create modified transition with internal IDs
        let modified = EdgeData {
            from,
            to,
            edge_type: transition.edge_type,
            label: transition.label,
            style: transition.style.clone(),
        };
        self.transitions.push(modified);
        Ok(())
    }

    /// Check if diagram has a start terminal
    pub fn has_start_terminal(&self) -> bool {
        self.has_start
    }

    /// Check if diagram has an end terminal
    pub fn has_end_terminal(&self) -> bool {
        self.has_end
    }

    /// Get all states
    pub fn states(&self) -> &[NodeData] {
        &self.states
    }

    /// Get all transitions
    pub fn transitions(&self) -> &[EdgeData] {
        &self.transitions
    }

    /// Get state count
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    /// Get transition count
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Get state index (for layout)
    pub fn state_index(&self, id: &str) -> Option<usize> {
        self.states.iter().position(|s| s.id == id)
    }

    /// Clear all data
    pub fn clear_all(&mut self) {
        self.states.clear();
        self.transitions.clear();
    }
}

impl Database for StateDatabase {
    type Node = NodeData;
    type Edge = EdgeData;

    fn add_node(&mut self, node: Self::Node) -> Result<()> {
        self.add_state(node)
    }

    fn add_edge(&mut self, edge: Self::Edge) -> Result<()> {
        self.add_transition(edge)
    }

    fn get_node(&self, id: &str) -> Option<&Self::Node> {
        self.states.iter().find(|s| s.id == id)
    }

    fn nodes(&self) -> impl Iterator<Item = &Self::Node> {
        self.states.iter()
    }

    fn edges(&self) -> impl Iterator<Item = &Self::Edge> {
        self.transitions.iter()
    }

    fn clear(&mut self) {
        self.clear_all()
    }

    fn node_count(&self) -> usize {
        self.state_count()
    }

    fn edge_count(&self) -> usize {
        self.transition_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EdgeType;

    #[test]
    fn test_add_state() {
        let mut db = StateDatabase::new();
        db.add_state(NodeData::new("Idle", "Idle")).unwrap();
        db.add_state(NodeData::new("Running", "Running")).unwrap();
        assert_eq!(db.state_count(), 2);
    }

    #[test]
    fn test_no_duplicate_states() {
        let mut db = StateDatabase::new();
        db.add_state(NodeData::new("Idle", "Idle")).unwrap();
        db.add_state(NodeData::new("Idle", "Idle")).unwrap();
        assert_eq!(db.state_count(), 1);
    }

    #[test]
    fn test_add_transition_creates_implicit_states() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("Idle", "Running")).unwrap();
        assert_eq!(db.state_count(), 2);
        assert_eq!(db.transition_count(), 1);
    }

    #[test]
    fn test_start_terminal_converted() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("[*]", "Idle")).unwrap();

        // [*] as source should become [*]_start
        assert!(db.has_start_terminal());
        assert!(db.get_node(START_TERMINAL).is_some());
        assert_eq!(
            db.get_node(START_TERMINAL).unwrap().shape,
            NodeShape::Terminal
        );
    }

    #[test]
    fn test_end_terminal_converted() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("Done", "[*]")).unwrap();

        // [*] as target should become [*]_end
        assert!(db.has_end_terminal());
        assert!(db.get_node(END_TERMINAL).is_some());
        assert_eq!(
            db.get_node(END_TERMINAL).unwrap().shape,
            NodeShape::Terminal
        );
    }

    #[test]
    fn test_both_terminals_separate() {
        let mut db = StateDatabase::new();
        db.add_transition(EdgeData::new("[*]", "Idle")).unwrap();
        db.add_transition(EdgeData::new("Idle", "[*]")).unwrap();

        // Should have 3 states: start, Idle, end
        assert_eq!(db.state_count(), 3);
        assert!(db.has_start_terminal());
        assert!(db.has_end_terminal());
        assert!(db.get_node(START_TERMINAL).is_some());
        assert!(db.get_node(END_TERMINAL).is_some());
    }

    #[test]
    fn test_transition_with_label() {
        let mut db = StateDatabase::new();
        let edge = EdgeData::with_label("Idle", "Running", EdgeType::Arrow, "start");
        db.add_transition(edge).unwrap();

        let transition = &db.transitions()[0];
        assert_eq!(transition.label, Some("start".to_string()));
    }
}
