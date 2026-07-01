//! Flowchart parser using chumsky
//!
//! Parses individual Mermaid.js flowchart statements into AST structures.

use super::whitespace::optional_whitespace;
use crate::core::{Direction, EdgeType, NodeShape, StyleDefinition};
use anyhow::Result;
use chumsky::prelude::*;
use chumsky::text::ident;

/// Chumsky-based flowchart parser
pub struct ChumskyFlowchartParser;

impl ChumskyFlowchartParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a single statement
    pub fn parse_statement(&self, input: &str) -> Result<Statement> {
        let parser = Self::statement_parser().then_ignore(end());

        parser
            .parse(input)
            .into_result()
            .map_err(|errors| anyhow::anyhow!("Parse errors: {:?}", errors))
    }

    /// Parse a graph declaration header (e.g., "graph TD" or "flowchart LR")
    pub fn parse_header(&self, input: &str) -> Option<Direction> {
        let trimmed = input.trim();

        // Handle semicolon-separated input like "graph LR; A-->B"
        let header_part = trimmed.split(';').next().unwrap_or(trimmed).trim();

        // Try to match "graph <dir>" or "flowchart <dir>"
        let parts: Vec<&str> = header_part.split_whitespace().collect();
        if parts.len() >= 2 {
            let keyword = parts[0].to_lowercase();
            if keyword == "graph" || keyword == "flowchart" {
                return parts[1].parse().ok();
            }
        }

        // Default to TopDown if just "graph" or "flowchart"
        if header_part.eq_ignore_ascii_case("graph")
            || header_part.eq_ignore_ascii_case("flowchart")
        {
            return Some(Direction::TopDown);
        }

        None
    }

    fn statement_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        recursive(|statements| {
            // Style directives should be tried first (they have distinctive keywords)
            Self::classdef_parser()
                .or(Self::style_parser())
                .or(Self::class_parser())
                .or(Self::linkstyle_parser())
                .or(Self::subgraph_parser(statements.clone()))
                .or(Self::edge_parser().map(Statement::Edge))
                .or(Self::node_parser().map(Statement::Node))
        })
    }

    /// Parse `classDef className fill:#f9f,stroke:#333`
    fn classdef_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        just("classDef")
            .then(optional_whitespace())
            .ignore_then(ident().map(|s: &str| s.to_string()))
            .then_ignore(optional_whitespace())
            .then(Self::style_string_parser())
            .map(|(name, style_str)| Statement::ClassDef(name, StyleDefinition::parse(&style_str)))
    }

    /// Parse `style nodeId1,nodeId2 fill:#f9f,stroke:#333`
    fn style_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        just("style")
            .then(optional_whitespace())
            .ignore_then(Self::id_list_parser())
            .then_ignore(optional_whitespace())
            .then(Self::style_string_parser())
            .map(|(node_ids, style_str)| {
                Statement::Style(node_ids, StyleDefinition::parse(&style_str))
            })
    }

    /// Parse `class nodeId1,nodeId2 className`
    fn class_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        just("class")
            .then(optional_whitespace())
            .ignore_then(Self::id_list_parser())
            .then_ignore(optional_whitespace())
            .then(ident().map(|s: &str| s.to_string()))
            .map(|(node_ids, class_name)| Statement::Class(node_ids, class_name))
    }

    /// Parse `linkStyle 0,1,2 stroke:#ff3`
    fn linkstyle_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        just("linkStyle")
            .then(optional_whitespace())
            .ignore_then(Self::index_list_parser())
            .then_ignore(optional_whitespace())
            .then(Self::style_string_parser())
            .map(|(indices, style_str)| {
                Statement::LinkStyle(indices, StyleDefinition::parse(&style_str))
            })
    }

    /// Parse a comma-separated list of identifiers: `A,B,C`
    fn id_list_parser<'src>() -> impl Parser<'src, &'src str, Vec<String>> + Clone {
        ident()
            .map(|s: &str| s.to_string())
            .separated_by(just(',').padded_by(optional_whitespace()))
            .at_least(1)
            .collect()
    }

    /// Parse a comma-separated list of indices: `0,1,2`
    fn index_list_parser<'src>() -> impl Parser<'src, &'src str, Vec<usize>> + Clone {
        // Parse digits as string then convert
        one_of('0'..='9')
            .repeated()
            .at_least(1)
            .collect::<String>()
            .map(|s| s.parse::<usize>().unwrap_or(0))
            .separated_by(just(',').padded_by(optional_whitespace()))
            .at_least(1)
            .collect()
    }

    /// Parse a style string: `fill:#f9f,stroke:#333,stroke-width:4px`
    fn style_string_parser<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        // Match everything except newlines and semicolons (statement separators)
        none_of("\n\r;")
            .repeated()
            .at_least(1)
            .collect::<String>()
            .map(|s| s.trim().to_string())
    }

    /// Parse `:::className` suffix for inline class application
    fn class_suffix_parser<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        just(":::").ignore_then(ident().map(|s: &str| s.to_string()))
    }

    fn node_parser<'src>() -> impl Parser<'src, &'src str, Node> + Clone {
        let node_id = ident()
            .map(|s: &str| s.to_string())
            .labelled("node identifier");

        // A[label] - Rectangle
        let rectangular = node_id
            .then_ignore(just('['))
            .then(Self::label_parser())
            .then_ignore(just(']'))
            .map(|(id, label)| (id, label, NodeShape::Rectangle));

        // A(label) - Rounded rectangle / stadium
        let rounded = node_id
            .then_ignore(just('('))
            .then(Self::label_parser())
            .then_ignore(just(')'))
            .map(|(id, label)| (id, label, NodeShape::RoundedRect));

        // A{label} - Diamond
        let diamond = node_id
            .then_ignore(just('{'))
            .then(Self::label_parser())
            .then_ignore(just('}'))
            .map(|(id, label)| (id, label, NodeShape::Diamond));

        // A((label)) - Circle
        let circle = node_id
            .then_ignore(just("(("))
            .then(Self::label_parser())
            .then_ignore(just("))"))
            .map(|(id, label)| (id, label, NodeShape::Circle));

        // A[[label]] - Subroutine
        let subroutine = node_id
            .then_ignore(just("[["))
            .then(Self::label_parser())
            .then_ignore(just("]]"))
            .map(|(id, label)| (id, label, NodeShape::Subroutine));

        // A{{label}} - Hexagon
        let hexagon = node_id
            .then_ignore(just("{{"))
            .then(Self::label_parser())
            .then_ignore(just("}}"))
            .map(|(id, label)| (id, label, NodeShape::Hexagon));

        // A[(label)] - Cylinder / database
        let cylinder = node_id
            .then_ignore(just("[("))
            .then(Self::label_parser())
            .then_ignore(just(")]"))
            .map(|(id, label)| (id, label, NodeShape::Cylinder));

        // A[/label/] - Parallelogram (input/output)
        let parallelogram = node_id
            .then_ignore(just("[/"))
            .then(Self::label_parser_no_slash())
            .then_ignore(just("/]"))
            .map(|(id, label)| (id, label, NodeShape::Parallelogram));

        // A[/label\] - Trapezoid
        let trapezoid = node_id
            .then_ignore(just("[/"))
            .then(Self::label_parser_no_slash())
            .then_ignore(just("\\]"))
            .map(|(id, label)| (id, label, NodeShape::Trapezoid));

        // A>label] - Asymmetric (flag)
        let asymmetric = node_id
            .then_ignore(just('>'))
            .then(Self::label_parser())
            .then_ignore(just(']'))
            .map(|(id, label)| (id, label, NodeShape::Asymmetric));

        // Combine all shapes, then optionally parse :::className suffix
        hexagon
            .or(cylinder)
            .or(subroutine)
            .or(circle)
            .or(parallelogram)
            .or(trapezoid)
            .or(rectangular)
            .or(rounded)
            .or(diamond)
            .or(asymmetric)
            .then(Self::class_suffix_parser().or_not())
            .map(|((id, label, shape), class)| Node {
                id,
                label,
                shape,
                class,
            })
            .labelled("node definition")
    }

    fn edge_parser<'src>() -> impl Parser<'src, &'src str, Edge> + Clone {
        let node_id = Self::node_reference();

        // Edge connectors - order by specificity (longer first)
        let thick_arrow = just("==>").to(EdgeType::ThickArrow);
        let thick_line = just("===").to(EdgeType::ThickLine);
        let dotted_arrow = just("-.->").to(EdgeType::DottedArrow);
        let dotted_line = just("-.-").to(EdgeType::DottedLine);
        let arrow = just("-->").to(EdgeType::Arrow);
        let line = just("---").to(EdgeType::Line);
        let open_arrow = just("--o").to(EdgeType::OpenArrow);
        let cross_arrow = just("--x").to(EdgeType::CrossArrow);
        let invisible = just("~~~").to(EdgeType::Invisible);

        let edge_connector = thick_arrow
            .or(thick_line)
            .or(dotted_arrow)
            .or(dotted_line)
            .or(arrow)
            .or(line)
            .or(open_arrow)
            .or(cross_arrow)
            .or(invisible)
            .then_ignore(optional_whitespace());

        // Edge label: |label|
        let edge_label = just('|')
            .ignore_then(Self::label_parser())
            .then_ignore(just('|'))
            .then_ignore(optional_whitespace())
            .or_not();

        node_id
            .clone()
            .then(edge_connector)
            .then(edge_label)
            .then(node_id)
            .map(|(((from_ref, edge_type), label), to_ref)| Edge {
                from: from_ref.id.clone(),
                to: to_ref.id.clone(),
                from_ref,
                to_ref,
                edge_type,
                label,
            })
            .labelled("edge definition")
    }

    fn node_reference<'src>() -> impl Parser<'src, &'src str, NodeRef> + Clone {
        ident()
            .map(|s: &str| s.to_string())
            .then(Self::label_suffix().or_not())
            .then(Self::class_suffix_parser().or_not())
            .map(|((id, shape_info), class)| {
                if let Some((label, shape)) = shape_info {
                    NodeRef {
                        id,
                        label: Some(label),
                        shape: Some(shape),
                        class,
                    }
                } else {
                    NodeRef {
                        id,
                        label: None,
                        shape: None,
                        class,
                    }
                }
            })
            .then_ignore(optional_whitespace())
    }

    fn label_suffix<'src>() -> impl Parser<'src, &'src str, (String, NodeShape)> + Clone {
        // Match node shape suffixes and extract label + shape
        let double_bracket = just("[[")
            .ignore_then(Self::label_parser())
            .then_ignore(just("]]"))
            .map(|label| (label, NodeShape::Subroutine));

        let double_brace = just("{{")
            .ignore_then(Self::label_parser())
            .then_ignore(just("}}"))
            .map(|label| (label, NodeShape::Hexagon));

        let double_paren = just("((")
            .ignore_then(Self::label_parser())
            .then_ignore(just("))"))
            .map(|label| (label, NodeShape::Circle));

        let cylinder = just("[(")
            .ignore_then(Self::label_parser())
            .then_ignore(just(")]"))
            .map(|label| (label, NodeShape::Cylinder));

        let bracket = just('[')
            .ignore_then(Self::label_parser())
            .then_ignore(just(']'))
            .map(|label| (label, NodeShape::Rectangle));

        let paren = just('(')
            .ignore_then(Self::label_parser())
            .then_ignore(just(')'))
            .map(|label| (label, NodeShape::RoundedRect));

        let brace = just('{')
            .ignore_then(Self::label_parser())
            .then_ignore(just('}'))
            .map(|label| (label, NodeShape::Diamond));

        let asymmetric = just('>')
            .ignore_then(Self::label_parser())
            .then_ignore(just(']'))
            .map(|label| (label, NodeShape::Asymmetric));

        let parallelogram = just("[/")
            .ignore_then(Self::label_parser_no_slash())
            .then_ignore(just("/]"))
            .map(|label| (label, NodeShape::Parallelogram));

        let trapezoid = just("[/")
            .ignore_then(Self::label_parser_no_slash())
            .then_ignore(just("\\]"))
            .map(|label| (label, NodeShape::Trapezoid));

        // Order by specificity
        double_bracket
            .or(double_brace)
            .or(double_paren)
            .or(cylinder)
            .or(parallelogram)
            .or(trapezoid)
            .or(bracket)
            .or(paren)
            .or(brace)
            .or(asymmetric)
    }

    fn subgraph_parser<'src, Statements>(
        statements: Statements,
    ) -> impl Parser<'src, &'src str, Statement> + Clone
    where
        Statements: Parser<'src, &'src str, Statement> + Clone + 'src,
    {
        let subgraph_title = just("subgraph")
            .then_ignore(optional_whitespace())
            .ignore_then(
                just('"')
                    .ignore_then(Self::label_parser())
                    .then_ignore(just('"'))
                    .or(Self::label_parser()),
            )
            .then_ignore(optional_whitespace());

        let subgraph_end = just("end").then_ignore(optional_whitespace()).ignored();

        let subgraph_statements = statements
            .clone()
            .then_ignore(optional_whitespace())
            .repeated()
            .collect();

        subgraph_title
            .then(subgraph_statements)
            .then_ignore(subgraph_end)
            .map(|(title, statements)| Statement::Subgraph(title, statements))
            .labelled("subgraph")
    }

    fn label_parser<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        none_of("[](){}|\"\n\r\t")
            .repeated()
            .at_least(1)
            .collect::<String>()
            .labelled("label")
    }

    fn label_parser_no_slash<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        none_of("[](){}|\"/\\\n\r\t")
            .repeated()
            .at_least(1)
            .collect::<String>()
            .labelled("label")
    }
}

impl Default for ChumskyFlowchartParser {
    fn default() -> Self {
        Self::new()
    }
}

/// A parsed node from the diagram
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    /// CSS class applied via `:::className` syntax
    pub class: Option<String>,
}

/// Node reference in an edge (ID + optional shape/label)
#[derive(Debug, Clone, PartialEq)]
pub struct NodeRef {
    pub id: String,
    pub label: Option<String>,
    pub shape: Option<NodeShape>,
    /// CSS class applied via `:::className` syntax
    pub class: Option<String>,
}

/// A parsed edge from the diagram
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub from_ref: NodeRef,
    pub to_ref: NodeRef,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// A parsed statement from the diagram
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Node(Node),
    Edge(Edge),
    Subgraph(String, Vec<Statement>),
    /// `classDef className fill:#f9f,stroke:#333`
    ClassDef(String, StyleDefinition),
    /// `style nodeId1,nodeId2 fill:#f9f,stroke:#333`
    Style(Vec<String>, StyleDefinition),
    /// `class nodeId1,nodeId2 className`
    Class(Vec<String>, String),
    /// `linkStyle 0,1,2 stroke:#ff3`
    LinkStyle(Vec<usize>, StyleDefinition),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let parser = ChumskyFlowchartParser::new();

        assert_eq!(parser.parse_header("graph TD"), Some(Direction::TopDown));
        assert_eq!(parser.parse_header("graph TB"), Some(Direction::TopDown));
        assert_eq!(parser.parse_header("graph LR"), Some(Direction::LeftRight));
        assert_eq!(parser.parse_header("graph RL"), Some(Direction::RightLeft));
        assert_eq!(parser.parse_header("graph BT"), Some(Direction::BottomUp));
        assert_eq!(
            parser.parse_header("flowchart LR"),
            Some(Direction::LeftRight)
        );
        assert_eq!(parser.parse_header("graph"), Some(Direction::TopDown));
        assert_eq!(parser.parse_header("not a graph"), None);
    }

    #[test]
    fn test_basic_node_parsing() {
        let parser = ChumskyFlowchartParser::new();

        let node = parser.parse_statement("A[Start]").unwrap();
        if let Statement::Node(node) = node {
            assert_eq!(node.id, "A");
            assert_eq!(node.label, "Start");
            assert_eq!(node.shape, NodeShape::Rectangle);
        } else {
            panic!("Expected node statement");
        }
    }

    #[test]
    fn test_node_shapes() {
        let parser = ChumskyFlowchartParser::new();

        // Rectangle
        let stmt = parser.parse_statement("A[Label]").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Rectangle,
                ..
            })
        ));

        // Rounded
        let stmt = parser.parse_statement("B(Label)").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::RoundedRect,
                ..
            })
        ));

        // Diamond
        let stmt = parser.parse_statement("C{Label}").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Diamond,
                ..
            })
        ));

        // Circle
        let stmt = parser.parse_statement("D((Label))").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Circle,
                ..
            })
        ));

        // Subroutine
        let stmt = parser.parse_statement("E[[Label]]").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Subroutine,
                ..
            })
        ));

        // Hexagon
        let stmt = parser.parse_statement("F{{Label}}").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Hexagon,
                ..
            })
        ));

        // Cylinder
        let stmt = parser.parse_statement("G[(Label)]").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Cylinder,
                ..
            })
        ));

        // Parallelogram
        let stmt = parser.parse_statement("H[/Label/]").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Parallelogram,
                ..
            })
        ));

        // Trapezoid
        let stmt = parser.parse_statement("I[/Label\\]").unwrap();
        assert!(matches!(
            stmt,
            Statement::Node(Node {
                shape: NodeShape::Trapezoid,
                ..
            })
        ));
    }

    #[test]
    fn test_edge_types() {
        let parser = ChumskyFlowchartParser::new();

        // Arrow
        let stmt = parser.parse_statement("A --> B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.edge_type, EdgeType::Arrow);
        } else {
            panic!("Expected edge");
        }

        // Line
        let stmt = parser.parse_statement("A --- B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.edge_type, EdgeType::Line);
        } else {
            panic!("Expected edge");
        }

        // Dotted arrow
        let stmt = parser.parse_statement("A -.-> B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.edge_type, EdgeType::DottedArrow);
        } else {
            panic!("Expected edge");
        }

        // Thick arrow
        let stmt = parser.parse_statement("A ==> B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.edge_type, EdgeType::ThickArrow);
        } else {
            panic!("Expected edge");
        }
    }

    #[test]
    fn test_edge_with_label() {
        let parser = ChumskyFlowchartParser::new();

        let stmt = parser.parse_statement("A -->|Yes| B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.from, "A");
            assert_eq!(edge.to, "B");
            assert_eq!(edge.label, Some("Yes".to_string()));
        } else {
            panic!("Expected edge");
        }
    }

    #[test]
    fn test_subgraph_parsing() {
        let parser = ChumskyFlowchartParser::new();

        let input = r#"subgraph "Process Group"
            A --> B
            B --> C
        end"#;

        let statement = parser.parse_statement(input).unwrap();
        if let Statement::Subgraph(title, children) = statement {
            assert_eq!(title, "Process Group");
            assert_eq!(children.len(), 2);
            assert!(children
                .iter()
                .all(|stmt| matches!(stmt, Statement::Edge(_))));
        } else {
            panic!("Expected subgraph statement");
        }
    }

    #[test]
    fn test_all_edge_connector_types() {
        let parser = ChumskyFlowchartParser::new();

        // Arrow
        let stmt = parser.parse_statement("A --> B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::Arrow,
                ..
            })
        ));

        // Thick arrow
        let stmt = parser.parse_statement("A ==> B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::ThickArrow,
                ..
            })
        ));

        // Line
        let stmt = parser.parse_statement("A --- B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::Line,
                ..
            })
        ));

        // Dotted line
        let stmt = parser.parse_statement("A -.- B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::DottedLine,
                ..
            })
        ));

        // Dotted arrow
        let stmt = parser.parse_statement("A -.-> B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::DottedArrow,
                ..
            })
        ));

        // Invisible
        let stmt = parser.parse_statement("A ~~~ B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::Invisible,
                ..
            })
        ));

        // Open arrow
        let stmt = parser.parse_statement("A --o B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::OpenArrow,
                ..
            })
        ));

        // Cross arrow
        let stmt = parser.parse_statement("A --x B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::CrossArrow,
                ..
            })
        ));

        // Thick line
        let stmt = parser.parse_statement("A === B").unwrap();
        assert!(matches!(
            stmt,
            Statement::Edge(Edge {
                edge_type: EdgeType::ThickLine,
                ..
            })
        ));
    }

    #[test]
    fn test_edge_with_inline_label_variations() {
        let parser = ChumskyFlowchartParser::new();

        // Standard inline label
        let stmt = parser.parse_statement("A -->|Yes| B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.label, Some("Yes".to_string()));
        } else {
            panic!("Expected edge");
        }

        // Label with spaces
        let stmt = parser.parse_statement("A -->|Hello World| B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.label, Some("Hello World".to_string()));
        } else {
            panic!("Expected edge");
        }

        // Label with special characters (allowed in labels)
        let stmt = parser.parse_statement("A -->|Yes/No| B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.label, Some("Yes/No".to_string()));
        } else {
            panic!("Expected edge");
        }
    }

    #[test]
    fn test_node_with_empty_label() {
        let parser = ChumskyFlowchartParser::new();

        // Node ID alone should work (no label)
        // But node with empty label brackets should fail (label parser requires at least 1 char)
        assert!(parser.parse_statement("A[]").is_err());

        // Node ID without brackets should work (implicit label = ID)
        // However, parse_statement expects a full statement, so just ID won't parse as a node
        // Let's test that a node with a valid label works
        let stmt = parser.parse_statement("A[Label]").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "A");
            assert_eq!(node.label, "Label");
        } else {
            panic!("Expected node");
        }
    }

    #[test]
    fn test_node_ids_with_numbers() {
        let parser = ChumskyFlowchartParser::new();

        // Node IDs can contain numbers after first char
        let stmt = parser.parse_statement("A1[Label]").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "A1");
            assert_eq!(node.label, "Label");
        } else {
            panic!("Expected node");
        }

        let stmt = parser.parse_statement("Node123[Test]").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "Node123");
        } else {
            panic!("Expected node");
        }
    }

    #[test]
    fn test_node_labels_with_special_chars() {
        let parser = ChumskyFlowchartParser::new();

        // Labels can contain various characters except delimiters
        let stmt = parser.parse_statement("A[Hello-World_123]").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.label, "Hello-World_123");
        } else {
            panic!("Expected node");
        }

        // Unicode characters
        let stmt = parser.parse_statement("A[こんにちは]").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.label, "こんにちは");
        } else {
            panic!("Expected node");
        }
    }

    #[test]
    fn test_subgraph_without_quotes() {
        let parser = ChumskyFlowchartParser::new();

        let input = r#"subgraph ProcessGroup
            A --> B
        end"#;

        let statement = parser.parse_statement(input).unwrap();
        if let Statement::Subgraph(title, _) = statement {
            assert_eq!(title, "ProcessGroup");
        } else {
            panic!("Expected subgraph statement");
        }
    }

    #[test]
    fn test_subgraph_with_nested_content() {
        let parser = ChumskyFlowchartParser::new();

        let input = r#"subgraph "Outer"
            A --> B
            subgraph "Inner"
                C --> D
            end
            B --> C
        end"#;

        let statement = parser.parse_statement(input).unwrap();
        if let Statement::Subgraph(title, children) = statement {
            assert_eq!(title, "Outer");
            assert!(children.len() >= 2);
        } else {
            panic!("Expected subgraph statement");
        }
    }

    #[test]
    fn test_whitespace_variations() {
        let parser = ChumskyFlowchartParser::new();

        // Extra whitespace
        assert!(parser.parse_statement("A  -->  B").is_ok());
        assert!(parser.parse_statement("A\t-->\tB").is_ok());
        assert!(parser.parse_statement("A\n-->\nB").is_ok());

        // No whitespace
        assert!(parser.parse_statement("A-->B").is_ok());
    }

    #[test]
    fn test_malformed_syntax_errors() {
        let parser = ChumskyFlowchartParser::new();

        // Missing closing bracket
        assert!(parser.parse_statement("A[Label").is_err());

        // Missing node ID
        assert!(parser.parse_statement("[Label]").is_err());

        // Incomplete edge
        assert!(parser.parse_statement("A -->").is_err());

        // Invalid connector
        assert!(parser.parse_statement("A ----> B").is_err());
    }

    #[test]
    fn test_edge_with_node_shapes() {
        let parser = ChumskyFlowchartParser::new();

        // Edge with shaped nodes
        let stmt = parser.parse_statement("A[Start] --> B{Decision}").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.from_ref.shape, Some(NodeShape::Rectangle));
            assert_eq!(edge.to_ref.shape, Some(NodeShape::Diamond));
        } else {
            panic!("Expected edge");
        }
    }

    #[test]
    fn test_multiple_statements_on_one_line() {
        use crate::core::{Database, Parser};

        // This tests the extract_statements function in parser.rs
        // Multiple statements separated by semicolons
        let input = "A --> B; B --> C; C --> D";
        // Note: parse_statement only parses one statement, so we test the full parser
        let full_parser = super::super::parser::FlowchartParser::new();
        let mut db = super::super::database::FlowchartDatabase::new();
        assert!(full_parser.parse(input, &mut db).is_ok());
        assert_eq!(db.edge_count(), 3);
    }

    #[test]
    fn test_parse_classdef() {
        use crate::core::Color;

        let parser = ChumskyFlowchartParser::new();
        let stmt = parser
            .parse_statement("classDef highlight fill:#f9f,stroke:#333")
            .unwrap();

        if let Statement::ClassDef(name, style) = stmt {
            assert_eq!(name, "highlight");
            assert_eq!(style.fill, Some(Color::Hex("#f9f".to_string())));
            assert_eq!(style.stroke, Some(Color::Hex("#333".to_string())));
        } else {
            panic!("Expected ClassDef statement");
        }
    }

    #[test]
    fn test_parse_style() {
        use crate::core::Color;

        let parser = ChumskyFlowchartParser::new();
        let stmt = parser
            .parse_statement("style A,B fill:#f00,color:#fff")
            .unwrap();

        if let Statement::Style(node_ids, style) = stmt {
            assert_eq!(node_ids, vec!["A", "B"]);
            assert_eq!(style.fill, Some(Color::Hex("#f00".to_string())));
            assert_eq!(style.text_color, Some(Color::Hex("#fff".to_string())));
        } else {
            panic!("Expected Style statement");
        }
    }

    #[test]
    fn test_parse_class() {
        let parser = ChumskyFlowchartParser::new();
        let stmt = parser.parse_statement("class A,B,C highlight").unwrap();

        if let Statement::Class(node_ids, class_name) = stmt {
            assert_eq!(node_ids, vec!["A", "B", "C"]);
            assert_eq!(class_name, "highlight");
        } else {
            panic!("Expected Class statement");
        }
    }

    #[test]
    fn test_parse_linkstyle() {
        use crate::core::Color;

        let parser = ChumskyFlowchartParser::new();
        let stmt = parser
            .parse_statement("linkStyle 0,1,2 stroke:#ff3,stroke-width:4px")
            .unwrap();

        if let Statement::LinkStyle(indices, style) = stmt {
            assert_eq!(indices, vec![0, 1, 2]);
            assert_eq!(style.stroke, Some(Color::Hex("#ff3".to_string())));
            assert_eq!(style.stroke_width, Some(4));
        } else {
            panic!("Expected LinkStyle statement");
        }
    }

    #[test]
    fn test_style_integration() {
        use crate::core::{Color, Database, Parser};

        let input = r#"
            graph TD
            classDef red fill:#f00
            A[Start] --> B[End]
            class A red
            style B fill:#0f0
        "#;

        let parser = super::super::parser::FlowchartParser::new();
        let mut db = super::super::database::FlowchartDatabase::new();
        parser.parse(input, &mut db).unwrap();

        // Check class was defined
        assert!(db.has_class("red"));
        let red_class = db.get_class("red").unwrap();
        assert_eq!(red_class.fill, Some(Color::Hex("#f00".to_string())));

        // Check class was applied to node A
        let node_a = db.get_node("A").unwrap();
        assert!(node_a.classes.contains(&"red".to_string()));

        // Check inline style was applied to node B
        let node_b = db.get_node("B").unwrap();
        assert!(node_b.inline_style.is_some());
        assert_eq!(
            node_b.inline_style.as_ref().unwrap().fill,
            Some(Color::Hex("#0f0".to_string()))
        );

        // Check resolved style for A combines class definition
        let resolved = db.resolve_node_style("A").unwrap();
        assert_eq!(resolved.fill, Some(Color::Hex("#f00".to_string())));
    }

    #[test]
    fn test_node_with_class_suffix() {
        let parser = ChumskyFlowchartParser::new();

        // Node with :::className suffix
        let stmt = parser.parse_statement("A[Start]:::highlight").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "A");
            assert_eq!(node.label, "Start");
            assert_eq!(node.shape, NodeShape::Rectangle);
            assert_eq!(node.class, Some("highlight".to_string()));
        } else {
            panic!("Expected node statement");
        }

        // Rounded node with class
        let stmt = parser.parse_statement("B(Label):::primary").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "B");
            assert_eq!(node.class, Some("primary".to_string()));
        } else {
            panic!("Expected node statement");
        }

        // Diamond with class
        let stmt = parser.parse_statement("C{Decision}:::warning").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "C");
            assert_eq!(node.shape, NodeShape::Diamond);
            assert_eq!(node.class, Some("warning".to_string()));
        } else {
            panic!("Expected node statement");
        }

        // Node without class still works
        let stmt = parser.parse_statement("D[NoClass]").unwrap();
        if let Statement::Node(node) = stmt {
            assert_eq!(node.id, "D");
            assert_eq!(node.class, None);
        } else {
            panic!("Expected node statement");
        }
    }

    #[test]
    fn test_edge_with_class_suffix_on_nodes() {
        let parser = ChumskyFlowchartParser::new();

        // Edge with class on target node
        let stmt = parser.parse_statement("A --> B[End]:::done").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.from, "A");
            assert_eq!(edge.to, "B");
            assert_eq!(edge.to_ref.class, Some("done".to_string()));
            assert_eq!(edge.from_ref.class, None);
        } else {
            panic!("Expected edge statement");
        }

        // Edge with class on source node (no shape, just ID + class)
        let stmt = parser.parse_statement("A:::start --> B").unwrap();
        if let Statement::Edge(edge) = stmt {
            assert_eq!(edge.from, "A");
            assert_eq!(edge.from_ref.class, Some("start".to_string()));
        } else {
            panic!("Expected edge statement");
        }
    }

    #[test]
    fn test_inline_class_integration() {
        use crate::core::{Color, Database, Parser};

        let input = r#"
            graph TD
            classDef green fill:#0f0
            A[Start]:::green --> B[End]
        "#;

        let parser = super::super::parser::FlowchartParser::new();
        let mut db = super::super::database::FlowchartDatabase::new();
        parser.parse(input, &mut db).unwrap();

        // Check class was applied via inline syntax
        let node_a = db.get_node("A").unwrap();
        assert!(node_a.classes.contains(&"green".to_string()));

        // Check resolved style for A uses class definition
        let resolved = db.resolve_node_style("A").unwrap();
        assert_eq!(resolved.fill, Some(Color::Hex("#0f0".to_string())));
    }
}
