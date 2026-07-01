//! Flowchart diagram plugin
//!
//! This module contains the flowchart diagram implementation for Mermaid.js
//! flowchart syntax support.

use crate::core::{Detector, Diagram};
use std::sync::Arc;

mod chumsky_parser;
mod database;
mod detector;
mod layout;
mod ordering;
mod parser;
mod renderer;
mod whitespace;

pub use database::*;
pub use detector::*;
pub use layout::*;
pub use parser::*;
pub use renderer::*;

/// Flowchart diagram implementation
pub struct FlowchartDiagram;

impl Diagram for FlowchartDiagram {
    type Database = FlowchartDatabase;
    type Parser = FlowchartParser;
    type Renderer = FlowchartRenderer;

    fn detector() -> Arc<dyn Detector> {
        Arc::new(FlowchartDetector::new())
    }

    fn create_parser() -> Self::Parser {
        FlowchartParser::new()
    }

    fn create_database() -> Self::Database {
        FlowchartDatabase::new()
    }

    fn create_renderer() -> Self::Renderer {
        FlowchartRenderer::new()
    }

    fn name() -> &'static str {
        "flowchart"
    }

    fn version() -> &'static str {
        "0.1.0"
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::core::{Database, Direction, LayoutAlgorithm, Parser, Renderer};

    #[test]
    fn test_full_pipeline() {
        // Integration test for the full flowchart pipeline

        // Test that we can create all components
        let detector = FlowchartDiagram::detector();
        let parser = FlowchartDiagram::create_parser();
        let mut database = FlowchartDiagram::create_database();
        let renderer = FlowchartDiagram::create_renderer();

        // Test detection
        let input = "graph TD\n    A --> B\n    B --> C";
        assert!(detector.detect(input));
        assert_eq!(detector.diagram_type(), "flowchart");

        // Test parsing
        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 3);
        assert_eq!(database.edge_count(), 2);
        assert_eq!(database.direction(), Direction::TopDown);

        // Test rendering - should produce ASCII output
        let output = renderer.render(&database).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_mermaid_compatibility() {
        // Test with real Mermaid.js syntax
        let input = r#"graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Process 1]
    B -->|No| D[Process 2]
    C --> E[End]
    D --> E"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let result = parser.parse(input, &mut database);
        assert!(result.is_ok());

        // Verify rich data is stored
        assert_eq!(database.direction(), Direction::TopDown);
        assert_eq!(
            database.get_node("A").unwrap().shape,
            crate::core::NodeShape::Rectangle
        );
        assert_eq!(
            database.get_node("B").unwrap().shape,
            crate::core::NodeShape::Diamond
        );
    }

    #[test]
    fn test_full_pipeline_with_all_shapes() {
        let input = r#"graph TD
    A[Rectangle] --> B(Rounded)
    B --> C{Diamond}
    C --> D((Circle))
    D --> E[[Subroutine]]
    E --> F{{Hexagon}}
    F --> G[(Cylinder)]
    G --> H[/Parallelogram/]
    H --> I[/Trapezoid\]
    I --> J>Asymmetric]"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.node_count(), 10);
        assert_eq!(layout_result.nodes.len(), 10);
        assert!(!output.is_empty());
        // Verify all node labels appear in output
        assert!(output.contains("Rectangle"));
        assert!(output.contains("Diamond"));
        assert!(output.contains("Circle"));
    }

    #[test]
    fn test_full_pipeline_with_all_edge_types() {
        let input = r#"graph LR
    A --> B
    B ==> C
    C --- D
    D -.- E
    E -.-> F
    F ~~~ G
    G --o H
    H --x I
    I === J"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.edge_count(), 9);
        assert_eq!(layout_result.edges.len(), 9);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_full_pipeline_with_edge_labels() {
        let input = r#"graph TD
    A[Start] -->|Path 1| B[Option A]
    A -->|Path 2| C[Option B]
    B -->|Success| D[End]
    C -->|Failure| D"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.edge_count(), 4);
        let edges: Vec<_> = database.edges().collect();
        assert_eq!(edges[0].label, Some("Path 1".to_string()));
        assert_eq!(edges[1].label, Some("Path 2".to_string()));

        assert_eq!(layout_result.edges.len(), 4);
        assert!(!output.is_empty());
        // Edge labels should appear in output
        assert!(output.contains("Path 1") || output.contains("Path 2"));
    }

    #[test]
    fn test_full_pipeline_with_subgraphs() {
        let input = r#"graph TD
    Start --> Process1
    subgraph "Group A"
        Process1 --> Process2
        Process2 --> Process3
    end
    Process3 --> End"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.edge_count(), 4);
        assert_eq!(database.node_count(), 5);
        assert_eq!(layout_result.nodes.len(), 5);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_full_pipeline_all_directions() {
        let directions = [
            ("graph TD", Direction::TopDown),
            ("graph BT", Direction::BottomUp),
            ("graph LR", Direction::LeftRight),
            ("graph RL", Direction::RightLeft),
        ];

        for (header, expected_dir) in directions {
            let input = format!("{}\n    A --> B\n    B --> C", header);

            let detector = FlowchartDetector::new();
            assert!(detector.detect(&input), "Failed detection for {}", header);

            let parser = FlowchartParser::new();
            let mut database = FlowchartDatabase::new();
            parser.parse(&input, &mut database).unwrap();
            assert_eq!(
                database.direction(),
                expected_dir,
                "Failed direction for {}",
                header
            );

            let layout = FlowchartLayoutAlgorithm::new();
            let layout_result = layout.layout(&database).unwrap();

            let renderer = FlowchartRenderer::new();
            let output = renderer.render(&database).unwrap();

            assert_eq!(layout_result.nodes.len(), 3);
            assert!(!output.is_empty(), "Empty output for {}", header);
        }
    }

    #[test]
    fn test_full_pipeline_complex_flowchart() {
        let input = r#"graph TD
    A[Start] --> B{Decision?}
    B -->|Yes| C[Process A]
    B -->|No| D[Process B]
    C --> E{Check A}
    D --> F{Check B}
    E -->|OK| G[End]
    E -->|Error| H[Fix A]
    F -->|OK| G
    F -->|Error| I[Fix B]
    H --> E
    I --> F"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.node_count(), 9);
        // Count edges: A->B, B->C, B->D, C->E, D->F, E->G, E->H, F->G, F->I, H->E, I->F = 11
        assert_eq!(database.edge_count(), 11);
        assert_eq!(layout_result.nodes.len(), 9);
        assert_eq!(layout_result.edges.len(), 11);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_full_pipeline_self_loop() {
        let input = r#"graph TD
    A[Process] --> A"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.node_count(), 1);
        assert_eq!(database.edge_count(), 1);
        assert_eq!(layout_result.nodes.len(), 1);
        assert_eq!(layout_result.edges.len(), 1);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_full_pipeline_disconnected_nodes() {
        let input = r#"graph TD
    A[Node A]
    B[Node B]
    C[Node C]"#;

        let detector = FlowchartDetector::new();
        assert!(detector.detect(input));

        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database).unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let layout_result = layout.layout(&database).unwrap();

        let renderer = FlowchartRenderer::new();
        let output = renderer.render(&database).unwrap();

        assert_eq!(database.node_count(), 3);
        assert_eq!(database.edge_count(), 0);
        assert_eq!(layout_result.nodes.len(), 3);
        assert_eq!(layout_result.edges.len(), 0);
        assert!(!output.is_empty());
    }
}
