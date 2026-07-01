//! State diagram plugin
//!
//! Implements state machine visualization with ASCII art.
//!
//! Syntax examples:
//! ```text
//! stateDiagram-v2
//!     [*] --> Idle
//!     Idle --> Processing : start
//!     Processing --> Done : complete
//!     Done --> [*]
//! ```

mod database;
mod detector;
mod layout;
mod parser;
mod renderer;

pub use database::{StateDatabase, END_TERMINAL, START_TERMINAL};
pub use detector::StateDetector;
pub use layout::{StateLayoutAlgorithm, StateLayoutResult};
pub use parser::StateParser;
pub use renderer::StateRenderer;

use crate::core::{Detector, Diagram};
use std::sync::Arc;

/// State diagram implementation
pub struct StateDiagram;

impl Diagram for StateDiagram {
    type Database = StateDatabase;
    type Parser = StateParser;
    type Renderer = StateRenderer;

    fn detector() -> Arc<dyn Detector> {
        Arc::new(StateDetector::new())
    }

    fn create_parser() -> Self::Parser {
        StateParser::new()
    }

    fn create_database() -> Self::Database {
        StateDatabase::new()
    }

    fn create_renderer() -> Self::Renderer {
        StateRenderer::new()
    }

    fn name() -> &'static str {
        "state"
    }

    fn version() -> &'static str {
        "0.1.0"
    }
}
