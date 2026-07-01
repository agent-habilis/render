//! Syntax parser abstraction trait
//!
//! This trait abstracts the parsing of diagram-specific syntax into
//! a common AST structure that can be converted to database operations.

use anyhow::Result;

/// Abstract syntax tree node for parsed syntax elements
#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxNode {
    /// A node/vertex in the diagram
    Node {
        id: String,
        label: Option<String>,
        metadata: SyntaxMetadata,
    },
    /// An edge/connection between nodes
    Edge {
        from: String,
        to: String,
        label: Option<String>,
        metadata: SyntaxMetadata,
    },
    /// A grouping construct (e.g., subgraph)
    Group {
        id: String,
        label: Option<String>,
        children: Vec<SyntaxNode>,
        metadata: SyntaxMetadata,
    },
}

/// Metadata that can be attached to syntax nodes
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyntaxMetadata {
    /// Additional key-value pairs for extensibility
    pub attributes: std::collections::HashMap<String, String>,
}

impl SyntaxMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.attributes.get(key)
    }
}

/// Trait for parsing diagram-specific syntax into a common AST
///
/// This trait abstracts the syntax parsing layer, allowing different
/// diagram types to have different syntax formats while sharing common
/// infrastructure for converting AST to database operations.
pub trait SyntaxParser: Send + Sync {
    /// Parse input text into a sequence of syntax nodes
    ///
    /// Returns a vector of syntax nodes representing the parsed structure.
    /// The parser should handle errors gracefully and return partial results
    /// when possible.
    fn parse(&self, input: &str) -> Result<Vec<SyntaxNode>>;

    /// Get the name of this syntax parser
    fn name(&self) -> &'static str;

    /// Get the version of this syntax parser
    fn version(&self) -> &'static str;

    /// Check if the input can be parsed by this syntax parser
    fn can_parse(&self, input: &str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_metadata() {
        let mut meta = SyntaxMetadata::new();
        meta = meta.with_attr("shape", "diamond");
        assert_eq!(meta.get("shape"), Some(&"diamond".to_string()));
        assert_eq!(meta.get("nonexistent"), None);
    }

    #[test]
    fn test_syntax_node_variants() {
        let node = SyntaxNode::Node {
            id: "A".to_string(),
            label: Some("Start".to_string()),
            metadata: SyntaxMetadata::new(),
        };

        let edge = SyntaxNode::Edge {
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            metadata: SyntaxMetadata::new(),
        };

        match node {
            SyntaxNode::Node { id, .. } => assert_eq!(id, "A"),
            _ => panic!("Expected Node variant"),
        }

        match edge {
            SyntaxNode::Edge { from, to, .. } => {
                assert_eq!(from, "A");
                assert_eq!(to, "B");
            }
            _ => panic!("Expected Edge variant"),
        }
    }
}
