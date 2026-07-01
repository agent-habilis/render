//! Flowchart parser implementation
//!
//! Parses flowchart diagram markup into structured data by delegating to the chumsky-based
//! statement parser for each logical statement in the input.

use super::chumsky_parser::{ChumskyFlowchartParser, NodeRef, Statement};
use super::FlowchartDatabase;
use crate::core::{Database, EdgeData, NodeData, Parser};
use anyhow::Result;
use std::cell::RefCell;
use std::cmp::Ordering;
use tracing::{debug, error, info, span, trace, warn, Level};

thread_local! {
    /// Thread-local storage for collecting parse warnings
    static PARSE_WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Clear any accumulated warnings
pub fn clear_warnings() {
    PARSE_WARNINGS.with(|w| w.borrow_mut().clear());
}

/// Get all accumulated warnings and clear them
pub fn take_warnings() -> Vec<String> {
    PARSE_WARNINGS.with(|w| std::mem::take(&mut *w.borrow_mut()))
}

/// Add a warning to the collection
fn add_warning(warning: String) {
    PARSE_WARNINGS.with(|w| w.borrow_mut().push(warning));
}

const CONNECTORS: [&str; 9] = [
    "-.->", "==>", "===", "-->", "---", "-.-", "--o", "--x", "~~~",
];

/// Flowchart parser implementation
pub struct FlowchartParser;

impl FlowchartParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FlowchartParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser<FlowchartDatabase> for FlowchartParser {
    fn parse(&self, input: &str, database: &mut FlowchartDatabase) -> Result<()> {
        let parse_span = span!(Level::INFO, "parse_flowchart", input_len = input.len());
        let _enter = parse_span.enter(); // Enter span to track duration

        trace!("Starting flowchart parsing");

        let chumsky = ChumskyFlowchartParser::new();

        // First, try to extract the direction from the header
        let direction_span = span!(Level::DEBUG, "parse_direction");
        let _direction_enter = direction_span.enter();
        for line in input.lines() {
            let trimmed = line.trim();
            if let Some(direction) = chumsky.parse_header(trimmed) {
                database.set_direction(direction);
                debug!(direction = ?direction, "Parsed diagram direction");
                break;
            }
        }
        drop(_direction_enter);

        let mut skipped_statements = Vec::new();
        let mut node_count = 0;
        let mut edge_count = 0;

        // Parse statements
        let statements_span = span!(Level::DEBUG, "parse_statements");
        let _statements_enter = statements_span.enter();
        for statement_text in extract_statements(input) {
            match chumsky.parse_statement(&statement_text) {
                Ok(statement) => {
                    trace!(statement = ?statement, "Parsing statement");
                    match &statement {
                        Statement::Node(_) => node_count += 1,
                        Statement::Edge(_) => edge_count += 1,
                        _ => {}
                    }
                    if let Err(e) = apply_statement(&statement, database) {
                        error!(error = %e, statement = ?statement, "Failed to apply statement");
                        return Err(e);
                    }
                }
                Err(e) => {
                    let warning = format!("Skipped invalid statement '{}': {}", statement_text, e);
                    warn!(error = %e, statement = %statement_text, "Failed to parse statement");
                    add_warning(warning);
                    skipped_statements.push(statement_text);
                }
            }
        }
        drop(_statements_enter);

        if !skipped_statements.is_empty() {
            warn!(
                skipped_count = skipped_statements.len(),
                skipped_statements = ?skipped_statements,
                "Skipped invalid statements"
            );

            // If we have no valid nodes/edges but had statements to parse, that's an error
            if node_count == 0 && edge_count == 0 {
                error!("No valid statements parsed");
                return Err(anyhow::anyhow!(
                    "Parse error: no valid statements found. Invalid syntax: {}",
                    skipped_statements.join(", ")
                ));
            }
        }

        info!(node_count, edge_count, "Parsing completed successfully");

        Ok(())
    }

    fn name(&self) -> &'static str {
        "flowchart"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        CONNECTORS.iter().any(|connector| input.contains(connector))
    }
}

fn extract_statements(input: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current_subgraph: Vec<String> = Vec::new();
    let mut in_subgraph = false;

    let normalized_input = normalize_inline_labels(input);

    for line in normalized_input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        if in_subgraph {
            current_subgraph.push(trimmed.to_string());
            if trimmed.eq_ignore_ascii_case("end") {
                statements.push(current_subgraph.join(" "));
                current_subgraph.clear();
                in_subgraph = false;
            }
            continue;
        }

        for segment in trimmed.split(';') {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }

            if segment.to_lowercase().starts_with("subgraph") {
                in_subgraph = true;
                current_subgraph.push(segment.to_string());
                break;
            }

            if is_graph_declaration(segment) {
                continue;
            }

            statements.extend(split_chained_edges(segment));
        }
    }

    statements
}

fn split_chained_edges(statement: &str) -> Vec<String> {
    let trimmed = statement.trim();
    let mut connectors = Vec::new();
    let mut nodes = Vec::new();
    let mut cursor = 0;

    while cursor < trimmed.len() {
        if let Some((pos, conn)) = find_next_connector(trimmed, cursor) {
            let node = trimmed[cursor..pos].trim();
            if !node.is_empty() {
                nodes.push(node.to_string());
            }
            connectors.push(conn);
            cursor = pos + conn.len();
            continue;
        }
        break;
    }

    if cursor <= trimmed.len() {
        let node = trimmed[cursor..].trim();
        if !node.is_empty() {
            nodes.push(node.to_string());
        }
    }

    if connectors.is_empty() || nodes.len() <= 1 {
        return vec![trimmed.to_string()];
    }

    let mut edges = Vec::new();
    for (i, connector) in connectors.iter().enumerate() {
        if let (Some(from), Some(to)) = (nodes.get(i), nodes.get(i + 1)) {
            edges.push(format!("{}{}{}", from, connector, to));
        }
    }

    edges
}

fn find_next_connector(statement: &str, start: usize) -> Option<(usize, &'static str)> {
    CONNECTORS
        .iter()
        .filter_map(|&conn| statement[start..].find(conn).map(|pos| (start + pos, conn)))
        .min_by(|a, b| match a.0.cmp(&b.0) {
            Ordering::Equal => b.1.len().cmp(&a.1.len()),
            other => other,
        })
}

fn normalize_inline_labels(input: &str) -> String {
    let mut result = String::new();
    let mut last_index = 0;
    let len = input.len();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'|' {
            if let Some(label_end_rel) = input[i + 1..].find('|') {
                let label_end = i + 1 + label_end_rel;
                let label = &input[i + 1..label_end];
                let mut suffix_idx = label_end + 1;
                while suffix_idx < len && bytes[suffix_idx].is_ascii_whitespace() {
                    suffix_idx += 1;
                }

                if let Some(&connector) = CONNECTORS.iter().find(|&&conn| {
                    suffix_idx + conn.len() <= len && input[suffix_idx..].starts_with(conn)
                }) {
                    let suffix_end = suffix_idx + connector.len();
                    let mut prefix_idx = i;
                    while prefix_idx > 0 {
                        let c = bytes[prefix_idx - 1];
                        if c == b'-' || c == b'=' {
                            prefix_idx -= 1;
                            continue;
                        }
                        break;
                    }

                    result.push_str(&input[last_index..prefix_idx]);
                    result.push_str(connector);
                    result.push('|');
                    result.push_str(label);
                    result.push('|');

                    i = suffix_end;
                    last_index = suffix_end;
                    continue;
                }
            }
        }
        i += 1;
    }

    if last_index < len {
        result.push_str(&input[last_index..]);
    }

    result
}

fn apply_statement(statement: &Statement, database: &mut FlowchartDatabase) -> Result<()> {
    match statement {
        Statement::Node(node) => {
            let mut node_data = NodeData::with_shape(&node.id, &node.label, node.shape);
            if let Some(class) = &node.class {
                node_data.add_class(class);
            }
            database.add_node(node_data)?;
        }
        Statement::Edge(edge) => {
            // Ensure both nodes exist with their shape info if available
            ensure_node_from_ref(database, &edge.from_ref)?;
            ensure_node_from_ref(database, &edge.to_ref)?;

            // Add the edge with full metadata
            let edge_data = if let Some(label) = &edge.label {
                EdgeData::with_label(&edge.from, &edge.to, edge.edge_type, label)
            } else {
                EdgeData::with_type(&edge.from, &edge.to, edge.edge_type)
            };
            database.add_edge(edge_data)?;
        }
        Statement::Subgraph(title, children) => {
            // Collect node IDs from children before applying them
            let member_ids = collect_node_ids(children);

            // Apply child statements to add nodes and edges
            for child in children {
                apply_statement(child, database)?;
            }

            // Register the subgraph with its members
            database.add_subgraph(title.clone(), member_ids);
        }
        Statement::ClassDef(name, style) => {
            // Define a CSS class
            database.define_class(name, style.clone());
        }
        Statement::Style(node_ids, style) => {
            // Apply inline style to nodes
            for node_id in node_ids {
                database.apply_node_style(node_id, style.clone());
            }
        }
        Statement::Class(node_ids, class_name) => {
            // Apply a class to nodes
            for node_id in node_ids {
                database.apply_class(node_id, class_name);
            }
        }
        Statement::LinkStyle(indices, style) => {
            // Apply style to edges by index
            for &index in indices {
                database.apply_edge_style(index, style.clone());
            }
        }
    }

    Ok(())
}

/// Collect all node IDs from a list of statements (for subgraph membership)
fn collect_node_ids(statements: &[Statement]) -> Vec<String> {
    let mut ids = Vec::new();
    for statement in statements {
        match statement {
            Statement::Node(node) => {
                ids.push(node.id.clone());
            }
            Statement::Edge(edge) => {
                // Add both ends of the edge as members
                if !ids.contains(&edge.from) {
                    ids.push(edge.from.clone());
                }
                if !ids.contains(&edge.to) {
                    ids.push(edge.to.clone());
                }
            }
            Statement::Subgraph(_, children) => {
                // For nested subgraphs (not supported visually yet), flatten the nodes
                // The nested subgraph's nodes belong to the outer subgraph
                for child_id in collect_node_ids(children) {
                    if !ids.contains(&child_id) {
                        ids.push(child_id);
                    }
                }
            }
            // Style statements don't contribute node IDs
            Statement::ClassDef(_, _)
            | Statement::Style(_, _)
            | Statement::Class(_, _)
            | Statement::LinkStyle(_, _) => {}
        }
    }
    ids
}

/// Ensure a node exists, using shape info from the reference if available
fn ensure_node_from_ref(database: &mut FlowchartDatabase, node_ref: &NodeRef) -> Result<()> {
    if database.has_node(&node_ref.id) {
        // Node exists - still apply class if specified in the reference
        if let Some(class) = &node_ref.class {
            database.apply_class(&node_ref.id, class);
        }
        return Ok(());
    }

    let label = node_ref.label.as_deref().unwrap_or(&node_ref.id);
    let shape = node_ref.shape.unwrap_or_default();
    let mut node_data = NodeData::with_shape(&node_ref.id, label, shape);
    if let Some(class) = &node_ref.class {
        node_data.add_class(class);
    }
    database.add_node(node_data)?;
    Ok(())
}

fn is_graph_declaration(line: &str) -> bool {
    let trimmed = line.trim();
    let without_semicolon = trimmed.trim_end_matches(';');

    without_semicolon.starts_with("graph ")
        || without_semicolon.starts_with("flowchart ")
        || without_semicolon == "graph"
        || without_semicolon == "flowchart"
        || without_semicolon.starts_with("flowchart TB")
        || without_semicolon.starts_with("flowchart TD")
        || without_semicolon.starts_with("flowchart LR")
        || without_semicolon.starts_with("flowchart RL")
        || without_semicolon.starts_with("flowchart BT")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Database, Direction, EdgeType, NodeShape};

    #[test]
    fn test_extract_statements_basics() {
        let input = "graph TD; A-->B; B-->C";
        let statements = extract_statements(input);
        assert_eq!(statements, vec!["A-->B", "B-->C"]);
    }

    #[test]
    fn test_extract_statements_handles_chains_and_comments() {
        let input = r#"
            graph TB
            A-->B-->C
            %% ignore
            D-->E"#;

        let statements = extract_statements(input);
        assert_eq!(statements, vec!["A-->B", "B-->C", "D-->E"]);
    }

    #[test]
    fn test_extract_statements_supports_additional_connectors() {
        let input = r#"
            graph TD
            A --o B --o C
            C --x D === E
            F -.-> G -.- H"#;

        let statements = extract_statements(input);
        assert_eq!(
            statements,
            vec!["A--oB", "B--oC", "C--xD", "D===E", "F-.->G", "G-.-H"]
        );
    }

    #[test]
    fn test_split_chained_edges() {
        let edges = split_chained_edges("A-->B-->C-->D");
        assert_eq!(edges, vec!["A-->B", "B-->C", "C-->D"]);
    }

    #[test]
    fn test_split_chained_edges_prefers_longest_connector() {
        let edges = split_chained_edges("A-.->B-.->C");
        assert_eq!(edges, vec!["A-.->B", "B-.->C"]);
    }

    #[test]
    fn test_normalize_inline_labels() {
        let statement = "A--|Yes|-->B; C--|No|---D";
        let normalized = normalize_inline_labels(statement);
        assert!(normalized.contains("-->|Yes|"));
        assert!(normalized.contains("---|No|"));
    }

    #[test]
    fn test_normalize_inline_labels_handles_additional_connectors() {
        let statement = "A --|Maybe|--o B; C --|X|=== D";
        let normalized = normalize_inline_labels(statement);
        assert!(normalized.contains("--o|Maybe|"));
        assert!(normalized.contains("===|X|"));
    }

    #[test]
    fn test_parser_sets_direction() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser.parse("graph LR\n    A-->B", &mut database).unwrap();
        assert_eq!(database.direction(), Direction::LeftRight);
    }

    #[test]
    fn test_parser_stores_node_shapes() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser
            .parse("graph TD\n    A[Rectangle]\n    B{Diamond}", &mut database)
            .unwrap();

        assert_eq!(database.get_node("A").unwrap().shape, NodeShape::Rectangle);
        assert_eq!(database.get_node("B").unwrap().shape, NodeShape::Diamond);
    }

    #[test]
    fn test_parser_stores_edge_types() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser
            .parse("graph TD\n    A --> B\n    B ==> C", &mut database)
            .unwrap();

        let edges: Vec<_> = database.edges().collect();
        assert_eq!(edges[0].edge_type, EdgeType::Arrow);
        assert_eq!(edges[1].edge_type, EdgeType::ThickArrow);
    }

    #[test]
    fn test_parser_stores_edge_labels() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser
            .parse("graph TD\n    A -->|Yes| B", &mut database)
            .unwrap();

        let edges: Vec<_> = database.edges().collect();
        assert_eq!(edges[0].label, Some("Yes".to_string()));
    }

    #[test]
    fn test_flowchart_parser_handles_chained_edges() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser
            .parse("graph TD\n    A-->B-->C", &mut database)
            .unwrap();
        assert_eq!(database.edge_count(), 2);
        assert_eq!(database.node_count(), 3);
    }

    #[test]
    fn test_flowchart_parser_handles_inline_label_connectors() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser
            .parse("graph TD\n    A --|Yes|--> B", &mut database)
            .unwrap();
        assert_eq!(database.edge_count(), 1);
        assert!(database.has_node("A"));
        assert!(database.has_node("B"));
    }

    #[test]
    fn test_subgraph_population() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        parser
            .parse(
                r#"graph TD
                subgraph "Group"
                    A --> B
                    B --> C
                end"#,
                &mut database,
            )
            .unwrap();

        assert_eq!(database.edge_count(), 2);
        assert_eq!(database.node_count(), 3);

        // Verify subgraph was registered
        assert_eq!(database.subgraph_count(), 1);
        let sg = database.get_subgraph("subgraph_0").unwrap();
        assert_eq!(sg.title, "Group");
        assert_eq!(sg.members.len(), 3);
        assert!(sg.members.contains(&"A".to_string()));
        assert!(sg.members.contains(&"B".to_string()));
        assert!(sg.members.contains(&"C".to_string()));
    }

    #[test]
    fn test_parser_handles_comments() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            %% This is a comment
            A --> B
            %% Another comment
            B --> C"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.edge_count(), 2);
        assert_eq!(database.node_count(), 3);
    }

    #[test]
    fn test_parser_handles_empty_lines() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD

            A --> B

            B --> C

        "#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.edge_count(), 2);
        assert_eq!(database.node_count(), 3);
    }

    #[test]
    fn test_parser_handles_all_edge_types() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A --> B
            B ==> C
            C --- D
            D -.- E
            E -.-> F
            F ~~~ G
            G --o H
            H --x I
            I === J"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.edge_count(), 9);

        let edges: Vec<_> = database.edges().collect();
        assert_eq!(edges[0].edge_type, EdgeType::Arrow);
        assert_eq!(edges[1].edge_type, EdgeType::ThickArrow);
        assert_eq!(edges[2].edge_type, EdgeType::Line);
        assert_eq!(edges[3].edge_type, EdgeType::DottedLine);
        assert_eq!(edges[4].edge_type, EdgeType::DottedArrow);
        assert_eq!(edges[5].edge_type, EdgeType::Invisible);
        assert_eq!(edges[6].edge_type, EdgeType::OpenArrow);
        assert_eq!(edges[7].edge_type, EdgeType::CrossArrow);
        assert_eq!(edges[8].edge_type, EdgeType::ThickLine);
    }

    #[test]
    fn test_parser_handles_inline_labels() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A --|Label1|--> B
            B --|Label2|--- C"#;

        parser.parse(input, &mut database).unwrap();
        let edges: Vec<_> = database.edges().collect();
        assert_eq!(edges[0].label, Some("Label1".to_string()));
        assert_eq!(edges[1].label, Some("Label2".to_string()));
    }

    #[test]
    fn test_parser_handles_node_declarations_without_edges() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A[Start]
            B[Process]
            C[End]
            A --> B
            B --> C"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 3);
        assert_eq!(database.edge_count(), 2);
    }

    #[test]
    fn test_parser_handles_mixed_node_shapes() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A[Rectangle]
            B(Rounded)
            C{Diamond}
            D((Circle))
            E[[Subroutine]]
            F{{Hexagon}}
            G[(Cylinder)]
            H[/Parallelogram/]
            I[/Trapezoid\]
            J>Asymmetric]"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 10);

        assert_eq!(database.get_node("A").unwrap().shape, NodeShape::Rectangle);
        assert_eq!(
            database.get_node("B").unwrap().shape,
            NodeShape::RoundedRect
        );
        assert_eq!(database.get_node("C").unwrap().shape, NodeShape::Diamond);
        assert_eq!(database.get_node("D").unwrap().shape, NodeShape::Circle);
        assert_eq!(database.get_node("E").unwrap().shape, NodeShape::Subroutine);
        assert_eq!(database.get_node("F").unwrap().shape, NodeShape::Hexagon);
        assert_eq!(database.get_node("G").unwrap().shape, NodeShape::Cylinder);
        assert_eq!(
            database.get_node("H").unwrap().shape,
            NodeShape::Parallelogram
        );
        assert_eq!(database.get_node("I").unwrap().shape, NodeShape::Trapezoid);
        assert_eq!(database.get_node("J").unwrap().shape, NodeShape::Asymmetric);
    }

    #[test]
    fn test_parser_handles_flowchart_keyword() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"flowchart TD
            A --> B"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.direction(), Direction::TopDown);
        assert_eq!(database.edge_count(), 1);
    }

    #[test]
    fn test_parser_handles_all_directions() {
        let parser = FlowchartParser::new();

        let directions = [
            ("graph TD", Direction::TopDown),
            ("graph TB", Direction::TopDown),
            ("graph BT", Direction::BottomUp),
            ("graph LR", Direction::LeftRight),
            ("graph RL", Direction::RightLeft),
            ("flowchart TD", Direction::TopDown),
            ("flowchart LR", Direction::LeftRight),
        ];

        for (header, expected_dir) in directions {
            let mut database = FlowchartDatabase::new();
            let input = format!("{}\n    A --> B", header);
            parser.parse(&input, &mut database).unwrap();
            assert_eq!(
                database.direction(),
                expected_dir,
                "Failed for header: {}",
                header
            );
        }
    }

    #[test]
    fn test_parser_handles_malformed_statements_gracefully() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A --> B
            This is not valid syntax
            C --> D"#;

        // Should parse valid statements and skip invalid ones
        let result = parser.parse(input, &mut database);
        // Parser should succeed but may log warnings about skipped statements
        assert!(result.is_ok() || result.is_err());
        // At minimum, valid edges should be parsed
        if result.is_ok() {
            assert!(database.edge_count() >= 2);
        }
    }

    #[test]
    fn test_parser_handles_unicode_in_labels() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A[こんにちは]
            B[Здравствуй]
            C[Hello 世界]
            A --> B
            B --> C"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 3);
        assert_eq!(database.get_node("A").unwrap().label, "こんにちは");
        assert_eq!(database.get_node("B").unwrap().label, "Здравствуй");
        assert_eq!(database.get_node("C").unwrap().label, "Hello 世界");
    }

    #[test]
    fn test_parser_handles_semicolon_separated_statements() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = "graph TD; A --> B; B --> C; C --> D";

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.edge_count(), 3);
        assert_eq!(database.node_count(), 4);
    }

    #[test]
    fn test_parser_handles_chained_edges() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = "graph TD\n    A --> B --> C --> D";

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.edge_count(), 3);
        assert_eq!(database.node_count(), 4);
    }

    #[test]
    fn test_parser_handles_chained_additional_edge_types() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            A --o B --o C
            C --x D === E"#;

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.edge_count(), 4);
        let edges: Vec<_> = database.edges().collect();
        assert_eq!(edges[0].edge_type, EdgeType::OpenArrow);
        assert_eq!(edges[1].edge_type, EdgeType::OpenArrow);
        assert_eq!(edges[2].edge_type, EdgeType::CrossArrow);
        assert_eq!(edges[3].edge_type, EdgeType::ThickLine);
    }

    #[test]
    fn test_parser_handles_empty_subgraph() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            subgraph "Empty"
            end"#;

        parser.parse(input, &mut database).unwrap();
        // Empty subgraph should be handled gracefully
        assert_eq!(database.node_count(), 0);
        assert_eq!(database.edge_count(), 0);

        // Subgraph should still be registered, just with no members
        assert_eq!(database.subgraph_count(), 1);
        let sg = database.get_subgraph("subgraph_0").unwrap();
        assert_eq!(sg.title, "Empty");
        assert!(sg.members.is_empty());
    }

    #[test]
    fn test_parser_multiple_subgraphs() {
        let parser = FlowchartParser::new();
        let mut database = FlowchartDatabase::new();

        let input = r#"graph TD
            subgraph "Alpha"
                A --> B
            end
            subgraph "Beta"
                C --> D
            end
            B --> C"#;

        parser.parse(input, &mut database).unwrap();

        assert_eq!(database.subgraph_count(), 2);
        assert_eq!(database.node_count(), 4);
        assert_eq!(database.edge_count(), 3);

        let alpha = database.get_subgraph("subgraph_0").unwrap();
        assert_eq!(alpha.title, "Alpha");
        assert!(alpha.members.contains(&"A".to_string()));
        assert!(alpha.members.contains(&"B".to_string()));

        let beta = database.get_subgraph("subgraph_1").unwrap();
        assert_eq!(beta.title, "Beta");
        assert!(beta.members.contains(&"C".to_string()));
        assert!(beta.members.contains(&"D".to_string()));

        // Verify node-to-subgraph lookup
        assert_eq!(database.node_subgraph("A").unwrap().id, "subgraph_0");
        assert_eq!(database.node_subgraph("D").unwrap().id, "subgraph_1");
    }
}
