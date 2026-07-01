//! Core parser trait for diagram markup
//!
//! This trait defines the interface for parsing diagram markup language
//! into structured data that can be stored in a database.

use anyhow::Result;

use super::Database;

/// Core trait for diagram parsers
///
/// This trait represents the parsing layer that converts diagram markup
/// into structured data. Each diagram type has its own parser implementation.
///
/// # Example
/// ```
/// use figurehead::core::{Parser, Database};
/// use figurehead::plugins::flowchart::{FlowchartParser, FlowchartDatabase};
///
/// let parser = FlowchartParser::new();
/// let mut db = FlowchartDatabase::new();
/// parser.parse("A --> B", &mut db).unwrap();
/// ```
pub trait Parser<D: Database>: Send + Sync {
    /// Parse diagram markup into the provided database
    fn parse(&self, input: &str, database: &mut D) -> Result<()>;

    /// Get the name of this parser
    fn name(&self) -> &'static str;

    /// Get the version of this parser
    fn version(&self) -> &'static str;

    /// Check if the input can be parsed by this parser
    fn can_parse(&self, input: &str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::flowchart::*;

    #[test]
    fn test_diagram_parser_trait_exists() {
        // This test ensures we have the DiagramParser trait
        let parser = FlowchartParser::new();
        assert_eq!(parser.name(), "flowchart");
        assert_eq!(parser.version(), "0.1.0");
    }

    #[test]
    fn test_parser_can_parse() {
        let parser = FlowchartParser::new();
        assert!(parser.can_parse("A --> B"));
        assert!(!parser.can_parse("some other text"));
    }

    #[test]
    fn test_basic_parsing() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser.parse("A --> B", &mut database).unwrap();
        assert_eq!(database.node_count(), 2);
        assert_eq!(database.edge_count(), 1);
    }
}
