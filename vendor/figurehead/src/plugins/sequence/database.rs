//! Sequence diagram database implementation
//!
//! Stores participants and messages for sequence diagrams.

use crate::core::Database;
use anyhow::Result;

/// Line style for message arrows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dotted,
}

/// Arrow head style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowHead {
    /// Filled arrow head (>>)
    Arrow,
    /// Open arrow head ()) - async
    Open,
    /// No arrow head (>)
    None,
}

/// Complete arrow type combining line and head style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrowType {
    pub line: LineStyle,
    pub head: ArrowHead,
}

impl ArrowType {
    pub fn solid_arrow() -> Self {
        Self {
            line: LineStyle::Solid,
            head: ArrowHead::Arrow,
        }
    }

    pub fn dotted_arrow() -> Self {
        Self {
            line: LineStyle::Dotted,
            head: ArrowHead::Arrow,
        }
    }

    pub fn solid_open() -> Self {
        Self {
            line: LineStyle::Solid,
            head: ArrowHead::None,
        }
    }

    pub fn dotted_open() -> Self {
        Self {
            line: LineStyle::Dotted,
            head: ArrowHead::None,
        }
    }
}

impl Default for ArrowType {
    fn default() -> Self {
        Self::solid_arrow()
    }
}

/// A participant in the sequence diagram
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// Unique identifier used in messages
    pub id: String,
    /// Display label (may differ from id via "as" syntax)
    pub label: String,
}

impl Participant {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
        }
    }

    pub fn with_label(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A message between participants
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Source participant id
    pub from: String,
    /// Target participant id
    pub to: String,
    /// Message label
    pub label: String,
    /// Arrow style
    pub arrow: ArrowType,
    /// Nesting depth (0 = top level, >0 = inside loop/alt blocks)
    pub depth: usize,
}

impl Message {
    pub fn new(from: impl Into<String>, to: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            label: label.into(),
            arrow: ArrowType::default(),
            depth: 0,
        }
    }

    pub fn with_arrow(mut self, arrow: ArrowType) -> Self {
        self.arrow = arrow;
        self
    }

    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }
}

/// Block kind for future loop/alt support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Loop,
    Alt,
    Else,
    Opt,
    Par,
}

/// Sequence item - either a message or block marker
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceItem {
    Message(Message),
    BlockStart {
        kind: BlockKind,
        label: String,
        depth: usize,
    },
    BlockEnd {
        depth: usize,
    },
}

/// Sequence diagram database
#[derive(Debug, Default)]
pub struct SequenceDatabase {
    participants: Vec<Participant>,
    items: Vec<SequenceItem>,
}

impl SequenceDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a participant (maintains order)
    pub fn add_participant(&mut self, participant: Participant) -> Result<()> {
        // Don't add duplicates
        if !self.participants.iter().any(|p| p.id == participant.id) {
            self.participants.push(participant);
        }
        Ok(())
    }

    /// Add a participant by id only (creates implicit participant)
    pub fn ensure_participant(&mut self, id: &str) -> Result<()> {
        if !self.participants.iter().any(|p| p.id == id) {
            self.participants.push(Participant::new(id));
        }
        Ok(())
    }

    /// Add a message
    pub fn add_message(&mut self, message: Message) -> Result<()> {
        // Ensure participants exist
        self.ensure_participant(&message.from)?;
        self.ensure_participant(&message.to)?;
        self.items.push(SequenceItem::Message(message));
        Ok(())
    }

    /// Get all participants in order
    pub fn participants(&self) -> &[Participant] {
        &self.participants
    }

    /// Get all items (messages and blocks)
    pub fn items(&self) -> &[SequenceItem] {
        &self.items
    }

    /// Get only messages (filtering out block markers)
    pub fn messages(&self) -> impl Iterator<Item = &Message> {
        self.items.iter().filter_map(|item| match item {
            SequenceItem::Message(m) => Some(m),
            _ => None,
        })
    }

    /// Get participant count
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages().count()
    }

    /// Get participant index (for layout)
    pub fn participant_index(&self, id: &str) -> Option<usize> {
        self.participants.iter().position(|p| p.id == id)
    }

    /// Clear all data
    pub fn clear_all(&mut self) {
        self.participants.clear();
        self.items.clear();
    }
}

/// Database trait implementation for SequenceDatabase
///
/// Maps Participant to Node and Message to Edge for trait compatibility.
impl Database for SequenceDatabase {
    type Node = Participant;
    type Edge = Message;

    fn add_node(&mut self, node: Self::Node) -> Result<()> {
        self.add_participant(node)
    }

    fn add_edge(&mut self, edge: Self::Edge) -> Result<()> {
        self.add_message(edge)
    }

    fn get_node(&self, id: &str) -> Option<&Self::Node> {
        self.participants.iter().find(|p| p.id == id)
    }

    fn nodes(&self) -> impl Iterator<Item = &Self::Node> {
        self.participants.iter()
    }

    fn edges(&self) -> impl Iterator<Item = &Self::Edge> {
        self.messages()
    }

    fn clear(&mut self) {
        self.clear_all()
    }

    fn node_count(&self) -> usize {
        self.participant_count()
    }

    fn edge_count(&self) -> usize {
        self.message_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_participant() {
        let mut db = SequenceDatabase::new();
        db.add_participant(Participant::new("Alice")).unwrap();
        db.add_participant(Participant::new("Bob")).unwrap();
        assert_eq!(db.participant_count(), 2);
    }

    #[test]
    fn test_no_duplicate_participants() {
        let mut db = SequenceDatabase::new();
        db.add_participant(Participant::new("Alice")).unwrap();
        db.add_participant(Participant::new("Alice")).unwrap();
        assert_eq!(db.participant_count(), 1);
    }

    #[test]
    fn test_add_message_creates_implicit_participants() {
        let mut db = SequenceDatabase::new();
        db.add_message(Message::new("Alice", "Bob", "Hello"))
            .unwrap();
        assert_eq!(db.participant_count(), 2);
        assert_eq!(db.message_count(), 1);
    }

    #[test]
    fn test_participant_order_preserved() {
        let mut db = SequenceDatabase::new();
        db.add_participant(Participant::new("Charlie")).unwrap();
        db.add_message(Message::new("Alice", "Bob", "Hi")).unwrap();

        let names: Vec<_> = db.participants().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(names, vec!["Charlie", "Alice", "Bob"]);
    }

    #[test]
    fn test_participant_with_alias() {
        let mut db = SequenceDatabase::new();
        db.add_participant(Participant::with_label("A", "Alice"))
            .unwrap();
        assert_eq!(db.participants()[0].id, "A");
        assert_eq!(db.participants()[0].label, "Alice");
    }
}
