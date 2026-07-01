//! Figurehead - Convert Mermaid.js diagrams to ASCII art
//!
//! A library for parsing Mermaid.js flowchart syntax and rendering it as ASCII art.
//!
//! # Quick Start
//!
//! ```rust
//! use figurehead::render;
//!
//! let input = "graph LR; A-->B-->C";
//! let ascii = render(input).unwrap();
//! println!("{}", ascii);
//! ```
//!
//! # Advanced Usage
//!
//! For more control, use the individual components:
//!
//! ```rust
//! use figurehead::prelude::*;
//!
//! let input = "graph TD; A[Start] --> B{Decision}";
//!
//! // Parse into a database
//! let parser = FlowchartParser::new();
//! let mut database = FlowchartDatabase::new();
//! parser.parse(input, &mut database).unwrap();
//!
//! // Access the parsed data
//! assert_eq!(database.node_count(), 2);
//! assert_eq!(database.direction(), Direction::TopDown);
//!
//! // Render to ASCII
//! let renderer = FlowchartRenderer::new();
//! let ascii = renderer.render(&database).unwrap();
//! ```

pub mod core;
pub mod plugins;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use core::*;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::core::{
        CharacterSet, Database, Detector, DiamondStyle, Direction, EdgeData, EdgeType,
        LayoutAlgorithm, NodeData, NodeShape, Parser, RenderConfig, Renderer,
    };
    pub use crate::plugins::flowchart::{
        FlowchartDatabase, FlowchartDetector, FlowchartLayoutAlgorithm, FlowchartParser,
        FlowchartRenderer,
    };
}

/// Render Mermaid flowchart syntax to ASCII art
///
/// This is the simplest way to convert a Mermaid diagram to ASCII.
/// Uses the default Unicode character set for best appearance.
///
/// # Arguments
/// * `input` - Mermaid flowchart syntax (e.g., "graph LR; A-->B")
///
/// # Returns
/// * `Ok(String)` - The ASCII art representation
/// * `Err` - If parsing or rendering fails
///
/// # Example
/// ```rust
/// use figurehead::render;
///
/// let ascii = render("graph LR; A[Start]-->B[End]").unwrap();
/// assert!(ascii.contains("Start"));
/// assert!(ascii.contains("End"));
/// ```
pub fn render(input: &str) -> anyhow::Result<String> {
    use crate::plugins::orchestrator::Orchestrator;

    let mut orchestrator = Orchestrator::with_all_plugins();
    orchestrator.register_default_detectors();
    orchestrator.process(input)
}

/// Render Mermaid flowchart syntax with a specific character set
///
/// Allows control over which characters are used for rendering.
///
/// # Arguments
/// * `input` - Mermaid flowchart syntax (e.g., "graph LR; A-->B")
/// * `style` - The character set to use for rendering
///
/// # Returns
/// * `Ok(String)` - The ASCII art representation
/// * `Err` - If parsing or rendering fails
///
/// # Example
/// ```rust
/// use figurehead::{render_with_style, CharacterSet};
///
/// // Pure ASCII for maximum compatibility
/// let ascii = render_with_style("graph LR; A-->B", CharacterSet::Ascii).unwrap();
///
/// // Compact mode with single-glyph nodes
/// let compact = render_with_style("graph LR; A-->B", CharacterSet::Compact).unwrap();
/// ```
pub fn render_with_style(input: &str, style: CharacterSet) -> anyhow::Result<String> {
    use crate::core::{Parser as _, Renderer as _};
    use crate::plugins::flowchart::{FlowchartDatabase, FlowchartParser, FlowchartRenderer};

    let parser = FlowchartParser::new();
    let mut database = FlowchartDatabase::new();
    parser.parse(input, &mut database)?;

    let renderer = FlowchartRenderer::with_style(style);
    renderer.render(&database)
}

/// Parse Mermaid flowchart syntax into a database without rendering
///
/// Useful when you need to inspect or modify the parsed data before rendering.
///
/// # Example
/// ```rust
/// use figurehead::{parse, Direction};
/// use figurehead::prelude::Database;
///
/// let db = parse("graph TD; A-->B-->C").unwrap();
/// assert_eq!(db.node_count(), 3);
/// assert_eq!(db.edge_count(), 2);
/// assert_eq!(db.direction(), Direction::TopDown);
/// ```
pub fn parse(input: &str) -> anyhow::Result<plugins::flowchart::FlowchartDatabase> {
    use crate::core::Parser as _;
    use crate::plugins::flowchart::{FlowchartDatabase, FlowchartParser};

    let parser = FlowchartParser::new();
    let mut database = FlowchartDatabase::new();
    parser.parse(input, &mut database)?;
    Ok(database)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_flowchart() {
        let input = "graph TD\n    A --> B";
        let result = render(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_gitgraph() {
        let input = "gitGraph\n   commit\n   commit";
        let result = render(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_with_style_ascii() {
        let input = "graph LR\n    A --> B";
        let result = render_with_style(input, CharacterSet::Ascii);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_with_style_unicode() {
        let input = "graph TD\n    A --> B";
        let result = render_with_style(input, CharacterSet::Unicode);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_with_style_compact() {
        let input = "graph LR\n    A --> B";
        let result = render_with_style(input, CharacterSet::Compact);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_with_style_unicode_math() {
        let input = "graph TD\n    A --> B";
        let result = render_with_style(input, CharacterSet::UnicodeMath);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_parse_flowchart() {
        let input = "graph TD\n    A --> B --> C";
        let result = parse(input);
        assert!(result.is_ok());
        let db = result.unwrap();
        assert_eq!(db.node_count(), 3);
        assert_eq!(db.edge_count(), 2);
        assert_eq!(db.direction(), Direction::TopDown);
    }

    #[test]
    fn test_parse_flowchart_lr() {
        let input = "graph LR\n    A --> B";
        let result = parse(input);
        assert!(result.is_ok());
        let db = result.unwrap();
        assert_eq!(db.direction(), Direction::LeftRight);
    }

    #[test]
    fn test_render_sequence() {
        let input = "sequenceDiagram\n    Alice->>Bob: Hello";
        let result = render(input);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let output = result.unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }
}
