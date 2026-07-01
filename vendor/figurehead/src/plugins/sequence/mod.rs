//! Sequence diagram plugin
//!
//! Implements UML sequence diagram visualization with ASCII art.
//!
//! Syntax examples:
//! ```text
//! sequenceDiagram
//!     Alice->>Bob: Hello
//!     Bob-->>Alice: Hi there
//! ```

mod database;
mod detector;
mod layout;
mod parser;
mod renderer;

pub use database::SequenceDatabase;
pub use detector::SequenceDetector;
pub use layout::{SequenceLayoutAlgorithm, SequenceLayoutResult};
pub use parser::SequenceParser;
pub use renderer::SequenceRenderer;

use crate::core::{Detector, Diagram};
use std::sync::Arc;

/// Sequence diagram implementation
pub struct SequenceDiagram;

impl Diagram for SequenceDiagram {
    type Database = SequenceDatabase;
    type Parser = SequenceParser;
    type Renderer = SequenceRenderer;

    fn detector() -> Arc<dyn Detector> {
        Arc::new(SequenceDetector::new())
    }

    fn create_parser() -> Self::Parser {
        SequenceParser::new()
    }

    fn create_database() -> Self::Database {
        SequenceDatabase::new()
    }

    fn create_renderer() -> Self::Renderer {
        SequenceRenderer::new()
    }

    fn name() -> &'static str {
        "sequence"
    }

    fn version() -> &'static str {
        "0.1.0"
    }
}
