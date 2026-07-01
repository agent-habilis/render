//! Sequence diagram parser
//!
//! Parses sequence diagram syntax into the database.

use super::database::{ArrowHead, ArrowType, LineStyle, Message, Participant, SequenceDatabase};
use crate::core::Parser;
use anyhow::Result;

/// Sequence diagram parser
pub struct SequenceParser;

impl SequenceParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse an arrow type from syntax like "->>" or "-->>"
    fn parse_arrow(&self, arrow_str: &str) -> Option<ArrowType> {
        match arrow_str {
            "->>" => Some(ArrowType {
                line: LineStyle::Solid,
                head: ArrowHead::Arrow,
            }),
            "-->>" => Some(ArrowType {
                line: LineStyle::Dotted,
                head: ArrowHead::Arrow,
            }),
            "->" => Some(ArrowType {
                line: LineStyle::Solid,
                head: ArrowHead::None,
            }),
            "-->" => Some(ArrowType {
                line: LineStyle::Dotted,
                head: ArrowHead::None,
            }),
            "-)" => Some(ArrowType {
                line: LineStyle::Solid,
                head: ArrowHead::Open,
            }),
            "--)" => Some(ArrowType {
                line: LineStyle::Dotted,
                head: ArrowHead::Open,
            }),
            _ => None,
        }
    }

    /// Parse a message line like "Alice->>Bob: Hello"
    fn parse_message_line(&self, line: &str) -> Option<(String, String, String, ArrowType)> {
        // Try each arrow type (longest first to avoid partial matches)
        let arrow_patterns = ["-->>", "->>", "-->", "->", "--)", "-)"];

        for arrow_str in arrow_patterns {
            if let Some(arrow_pos) = line.find(arrow_str) {
                let from = line[..arrow_pos].trim().to_string();
                let rest = &line[arrow_pos + arrow_str.len()..];

                // Find the colon separating target from message
                if let Some(colon_pos) = rest.find(':') {
                    let to = rest[..colon_pos].trim().to_string();
                    let label = rest[colon_pos + 1..].trim().to_string();

                    if !from.is_empty() && !to.is_empty() {
                        if let Some(arrow) = self.parse_arrow(arrow_str) {
                            return Some((from, to, label, arrow));
                        }
                    }
                }
            }
        }
        None
    }

    /// Parse a participant line like "participant Alice" or "participant A as Alice"
    fn parse_participant_line(&self, line: &str) -> Option<Participant> {
        let line = line.trim();

        // Handle "participant X as Label" or "actor X as Label"
        let prefixes = ["participant ", "actor "];

        for prefix in prefixes {
            if let Some(rest) = line.strip_prefix(prefix) {
                let rest = rest.trim();

                // Check for "as" alias syntax
                if let Some(as_pos) = rest.find(" as ") {
                    let id = rest[..as_pos].trim().to_string();
                    let label = rest[as_pos + 4..].trim().to_string();
                    return Some(Participant::with_label(id, label));
                } else {
                    // Just an id
                    let id = rest.to_string();
                    return Some(Participant::new(id));
                }
            }
        }
        None
    }
}

impl Default for SequenceParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser<SequenceDatabase> for SequenceParser {
    fn parse(&self, input: &str, database: &mut SequenceDatabase) -> Result<()> {
        for line in input.lines() {
            let line = line.trim();

            // Skip empty lines and the diagram declaration
            if line.is_empty() || line.to_lowercase().starts_with("sequencediagram") {
                continue;
            }

            // Try to parse as participant declaration
            if let Some(participant) = self.parse_participant_line(line) {
                database.add_participant(participant)?;
                continue;
            }

            // Try to parse as message
            if let Some((from, to, label, arrow)) = self.parse_message_line(line) {
                let message = Message::new(from, to, label).with_arrow(arrow);
                database.add_message(message)?;
                continue;
            }

            // Unknown line - skip for now (could add warnings later)
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "sequence"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.contains("sequencediagram") || lower.contains("->>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser
            .parse("sequenceDiagram\n    Alice->>Bob: Hello", &mut db)
            .unwrap();

        assert_eq!(db.participant_count(), 2);
        assert_eq!(db.message_count(), 1);

        let msg = db.messages().next().unwrap();
        assert_eq!(msg.from, "Alice");
        assert_eq!(msg.to, "Bob");
        assert_eq!(msg.label, "Hello");
        assert_eq!(msg.arrow.line, LineStyle::Solid);
        assert_eq!(msg.arrow.head, ArrowHead::Arrow);
    }

    #[test]
    fn test_parse_dotted_arrow() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser
            .parse("sequenceDiagram\n    Bob-->>Alice: Response", &mut db)
            .unwrap();

        let msg = db.messages().next().unwrap();
        assert_eq!(msg.arrow.line, LineStyle::Dotted);
        assert_eq!(msg.arrow.head, ArrowHead::Arrow);
    }

    #[test]
    fn test_parse_open_arrows() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser
            .parse(
                "sequenceDiagram\n    A->B: No head\n    C-->D: Dotted no head",
                &mut db,
            )
            .unwrap();

        let messages: Vec<_> = db.messages().collect();
        assert_eq!(messages[0].arrow.head, ArrowHead::None);
        assert_eq!(messages[0].arrow.line, LineStyle::Solid);
        assert_eq!(messages[1].arrow.head, ArrowHead::None);
        assert_eq!(messages[1].arrow.line, LineStyle::Dotted);
    }

    #[test]
    fn test_parse_explicit_participant() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser
            .parse(
                "sequenceDiagram\n    participant Alice\n    participant Bob\n    Bob->>Alice: Hi",
                &mut db,
            )
            .unwrap();

        // Explicit participants come first
        let names: Vec<_> = db.participants().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(names, vec!["Alice", "Bob"]);
    }

    #[test]
    fn test_parse_participant_alias() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser.parse("sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: Hello", &mut db).unwrap();

        assert_eq!(db.participants()[0].id, "A");
        assert_eq!(db.participants()[0].label, "Alice");
        assert_eq!(db.participants()[1].id, "B");
        assert_eq!(db.participants()[1].label, "Bob");
    }

    #[test]
    fn test_parse_actor_keyword() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser
            .parse(
                "sequenceDiagram\n    actor User\n    User->>System: Request",
                &mut db,
            )
            .unwrap();

        assert_eq!(db.participant_count(), 2);
        assert_eq!(db.participants()[0].id, "User");
    }

    #[test]
    fn test_parse_multiple_messages() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        let input = r#"sequenceDiagram
            Alice->>Bob: Hello
            Bob-->>Alice: Hi there
            Alice->>Bob: How are you?"#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.message_count(), 3);
    }

    #[test]
    fn test_parse_async_arrow() {
        let parser = SequenceParser::new();
        let mut db = SequenceDatabase::new();

        parser
            .parse("sequenceDiagram\n    A-)B: Async message", &mut db)
            .unwrap();

        let msg = db.messages().next().unwrap();
        assert_eq!(msg.arrow.line, LineStyle::Solid);
        assert_eq!(msg.arrow.head, ArrowHead::Open);
    }
}
