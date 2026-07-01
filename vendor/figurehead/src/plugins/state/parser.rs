//! State diagram parser using chumsky
//!
//! Parses state diagram syntax into the database.

use super::database::StateDatabase;
use crate::core::{EdgeData, EdgeType, NodeData, NodeShape, Parser as CoreParser};
use anyhow::Result;
use chumsky::prelude::*;

/// Parsed state diagram statement
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// State declaration: `state "description" as id`
    StateDecl { id: String, label: String },
    /// Transition: `from --> to` or `from --> to : label`
    Transition {
        from: String,
        to: String,
        label: Option<String>,
    },
}

/// State diagram parser
pub struct StateParser;

impl StateParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a terminal state [*]
    fn terminal_parser<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, String, extra::Err<Rich<'src, char>>> + Clone {
        just("[*]").to("[*]".to_string())
    }

    /// Parse an identifier (state name)
    fn identifier<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, String, extra::Err<Rich<'src, char>>> + Clone {
        any()
            .filter(|c: &char| c.is_alphanumeric() || *c == '_')
            .repeated()
            .at_least(1)
            .collect::<String>()
    }

    /// Parse a state reference (either [*] or identifier)
    fn state_ref<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, String, extra::Err<Rich<'src, char>>> + Clone {
        Self::terminal_parser().or(Self::identifier())
    }

    /// Parse a quoted string
    fn quoted_string<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, String, extra::Err<Rich<'src, char>>> + Clone {
        just('"')
            .ignore_then(any().filter(|c| *c != '"').repeated().collect::<String>())
            .then_ignore(just('"'))
    }

    /// Parse a transition: `from --> to` or `from --> to : label`
    fn transition_parser<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, Statement, extra::Err<Rich<'src, char>>> + Clone
    {
        let ws = any()
            .filter(|c: &char| c.is_whitespace())
            .repeated()
            .collect::<String>();

        let label = just(':')
            .padded_by(ws)
            .ignore_then(any().filter(|c| *c != '\n').repeated().collect::<String>())
            .map(|s| s.trim().to_string())
            .or_not();

        Self::state_ref()
            .padded_by(ws)
            .then_ignore(just("-->"))
            .padded_by(ws)
            .then(Self::state_ref())
            .padded_by(ws)
            .then(label)
            .map(|((from, to), label)| Statement::Transition {
                from,
                to,
                label: label.filter(|s| !s.is_empty()),
            })
    }

    /// Parse a state declaration: `state "description" as id`
    fn state_decl_parser<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, Statement, extra::Err<Rich<'src, char>>> + Clone
    {
        let ws = any()
            .filter(|c: &char| c.is_whitespace())
            .repeated()
            .at_least(1)
            .collect::<String>();

        just("state")
            .ignore_then(ws)
            .ignore_then(Self::quoted_string())
            .then_ignore(ws)
            .then_ignore(just("as"))
            .then_ignore(ws)
            .then(Self::identifier())
            .map(|(label, id)| Statement::StateDecl { id, label })
    }

    /// Parse a single statement
    fn statement_parser<'src>(
    ) -> impl chumsky::Parser<'src, &'src str, Statement, extra::Err<Rich<'src, char>>> + Clone
    {
        Self::state_decl_parser().or(Self::transition_parser())
    }

    /// Parse a statement from input
    pub fn parse_statement(&self, input: &str) -> Result<Statement> {
        let ws = any()
            .filter(|c: &char| c.is_whitespace())
            .repeated()
            .collect::<String>();

        let parser = ws.ignore_then(Self::statement_parser()).then_ignore(end());

        parser
            .parse(input.trim())
            .into_result()
            .map_err(|errors| anyhow::anyhow!("Parse error: {:?}", errors))
    }

    /// Check if a line is a header line
    fn is_header_line(&self, line: &str) -> bool {
        let trimmed = line.trim().to_lowercase();
        trimmed.starts_with("statediagram")
    }

    /// Check if a line is a comment
    fn is_comment(&self, line: &str) -> bool {
        line.trim().starts_with("%%")
    }
}

impl Default for StateParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreParser<StateDatabase> for StateParser {
    fn parse(&self, input: &str, database: &mut StateDatabase) -> Result<()> {
        for line in input.lines() {
            let trimmed = line.trim();

            // Skip empty lines, comments, and header
            if trimmed.is_empty() || self.is_comment(trimmed) || self.is_header_line(trimmed) {
                continue;
            }

            // Try to parse the line
            match self.parse_statement(trimmed) {
                Ok(Statement::StateDecl { id, label }) => {
                    database.add_state(NodeData::with_shape(&id, &label, NodeShape::Rectangle))?;
                }
                Ok(Statement::Transition { from, to, label }) => {
                    let edge = match label {
                        Some(lbl) => EdgeData::with_label(&from, &to, EdgeType::Arrow, lbl),
                        None => EdgeData::new(&from, &to),
                    };
                    database.add_transition(edge)?;
                }
                Err(_) => {
                    // Skip unparseable lines for now
                    continue;
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "state"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        let trimmed = input.trim().to_lowercase();
        trimmed.starts_with("statediagram") || input.contains("[*]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_transition() {
        let parser = StateParser::new();
        let result = parser.parse_statement("Idle --> Running").unwrap();
        assert_eq!(
            result,
            Statement::Transition {
                from: "Idle".to_string(),
                to: "Running".to_string(),
                label: None,
            }
        );
    }

    #[test]
    fn test_parse_transition_with_label() {
        let parser = StateParser::new();
        let result = parser.parse_statement("Idle --> Running : start").unwrap();
        assert_eq!(
            result,
            Statement::Transition {
                from: "Idle".to_string(),
                to: "Running".to_string(),
                label: Some("start".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_terminal_transition() {
        let parser = StateParser::new();
        let result = parser.parse_statement("[*] --> Idle").unwrap();
        assert_eq!(
            result,
            Statement::Transition {
                from: "[*]".to_string(),
                to: "Idle".to_string(),
                label: None,
            }
        );
    }

    #[test]
    fn test_parse_state_declaration() {
        let parser = StateParser::new();
        let result = parser
            .parse_statement("state \"Processing data\" as s1")
            .unwrap();
        assert_eq!(
            result,
            Statement::StateDecl {
                id: "s1".to_string(),
                label: "Processing data".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_full_diagram() {
        let parser = StateParser::new();
        let mut db = StateDatabase::new();

        let input = r#"
stateDiagram-v2
    [*] --> Idle
    Idle --> Processing : start
    Processing --> Done : complete
    Done --> [*]
"#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.state_count(), 5); // [*]_start, Idle, Processing, Done, [*]_end
        assert_eq!(db.transition_count(), 4);
    }

    #[test]
    fn test_skips_comments() {
        let parser = StateParser::new();
        let mut db = StateDatabase::new();

        let input = r#"
stateDiagram-v2
    %% This is a comment
    [*] --> Idle
"#;

        parser.parse(input, &mut db).unwrap();
        assert_eq!(db.transition_count(), 1);
    }

    #[test]
    fn test_can_parse() {
        let parser = StateParser::new();
        assert!(parser.can_parse("stateDiagram-v2\n[*] --> Idle"));
        assert!(parser.can_parse("[*] --> Idle"));
        assert!(!parser.can_parse("graph TD\nA --> B"));
    }
}
