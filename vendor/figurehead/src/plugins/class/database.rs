//! Class diagram database
//!
//! Stores classes and relationships for class diagrams.

use crate::core::Database;
use anyhow::Result;

/// Visibility modifier for class members
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,    // +
    Private,   // -
    Protected, // #
    Package,   // ~
}

impl Visibility {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '+' => Some(Visibility::Public),
            '-' => Some(Visibility::Private),
            '#' => Some(Visibility::Protected),
            '~' => Some(Visibility::Package),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Visibility::Public => '+',
            Visibility::Private => '-',
            Visibility::Protected => '#',
            Visibility::Package => '~',
        }
    }
}

/// Classifier for methods (abstract, static)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classifier {
    Abstract, // *
    Static,   // $
}

impl Classifier {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '*' => Some(Classifier::Abstract),
            '$' => Some(Classifier::Static),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Classifier::Abstract => '*',
            Classifier::Static => '$',
        }
    }
}

/// A class member (attribute or method)
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub visibility: Option<Visibility>,
    pub name: String,
    pub member_type: Option<String>,
    pub classifier: Option<Classifier>,
    pub is_method: bool,
}

impl Member {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            visibility: None,
            name: name.into(),
            member_type: None,
            classifier: None,
            is_method: false,
        }
    }

    pub fn attribute(name: impl Into<String>) -> Self {
        Self::new(name)
    }

    pub fn method(name: impl Into<String>) -> Self {
        Self {
            visibility: None,
            name: name.into(),
            member_type: None,
            classifier: None,
            is_method: true,
        }
    }

    pub fn with_visibility(mut self, v: Visibility) -> Self {
        self.visibility = Some(v);
        self
    }

    pub fn with_type(mut self, t: impl Into<String>) -> Self {
        self.member_type = Some(t.into());
        self
    }

    pub fn with_classifier(mut self, c: Classifier) -> Self {
        self.classifier = Some(c);
        self
    }
}

/// A class in the diagram
#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub attributes: Vec<Member>,
    pub methods: Vec<Member>,
    pub annotation: Option<String>,
}

impl Class {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attributes: Vec::new(),
            methods: Vec::new(),
            annotation: None,
        }
    }

    pub fn with_annotation(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }

    pub fn add_attribute(&mut self, member: Member) {
        self.attributes.push(member);
    }

    pub fn add_method(&mut self, member: Member) {
        self.methods.push(member);
    }
}

/// Relationship type between classes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipKind {
    Inheritance, // <|--
    Composition, // *--
    Aggregation, // o--
    Association, // -->
    Dependency,  // ..>
    Realization, // ..|>
    Link,        // --
    DashedLink,  // ..
}

/// A relationship between classes
#[derive(Debug, Clone)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub kind: RelationshipKind,
    pub label: Option<String>,
    pub from_cardinality: Option<String>,
    pub to_cardinality: Option<String>,
}

impl Relationship {
    pub fn new(from: impl Into<String>, to: impl Into<String>, kind: RelationshipKind) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind,
            label: None,
            from_cardinality: None,
            to_cardinality: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Class diagram database
pub struct ClassDatabase {
    classes: Vec<Class>,
    relationships: Vec<Relationship>,
}

impl ClassDatabase {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            relationships: Vec::new(),
        }
    }

    pub fn add_class(&mut self, class: Class) -> Result<()> {
        self.classes.push(class);
        Ok(())
    }

    pub fn add_relationship(&mut self, rel: Relationship) -> Result<()> {
        self.relationships.push(rel);
        Ok(())
    }

    pub fn classes(&self) -> &[Class] {
        &self.classes
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    pub fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    pub fn get_class(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
    }

    pub fn get_class_mut(&mut self, name: &str) -> Option<&mut Class> {
        self.classes.iter_mut().find(|c| c.name == name)
    }

    /// Get or create a class by name
    pub fn get_or_create_class(&mut self, name: &str) -> &mut Class {
        if self.get_class(name).is_none() {
            self.classes.push(Class::new(name));
        }
        self.get_class_mut(name).unwrap()
    }
}

impl Default for ClassDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for ClassDatabase {
    type Node = Class;
    type Edge = Relationship;

    fn add_node(&mut self, node: Self::Node) -> Result<()> {
        self.add_class(node)
    }

    fn add_edge(&mut self, edge: Self::Edge) -> Result<()> {
        self.add_relationship(edge)
    }

    fn get_node(&self, id: &str) -> Option<&Self::Node> {
        self.get_class(id)
    }

    fn nodes(&self) -> impl Iterator<Item = &Self::Node> {
        self.classes.iter()
    }

    fn edges(&self) -> impl Iterator<Item = &Self::Edge> {
        self.relationships.iter()
    }

    fn clear(&mut self) {
        self.classes.clear();
        self.relationships.clear();
    }

    fn node_count(&self) -> usize {
        self.classes.len()
    }

    fn edge_count(&self) -> usize {
        self.relationships.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty_class() {
        let class = Class::new("Animal");
        assert_eq!(class.name, "Animal");
        assert!(class.attributes.is_empty());
        assert!(class.methods.is_empty());
        assert!(class.annotation.is_none());
    }

    #[test]
    fn test_add_attributes() {
        let mut class = Class::new("Person");
        class.add_attribute(
            Member::attribute("name")
                .with_visibility(Visibility::Public)
                .with_type("string"),
        );
        class.add_attribute(
            Member::attribute("age")
                .with_visibility(Visibility::Private)
                .with_type("int"),
        );

        assert_eq!(class.attributes.len(), 2);
        assert_eq!(class.attributes[0].name, "name");
        assert_eq!(class.attributes[0].visibility, Some(Visibility::Public));
        assert_eq!(class.attributes[0].member_type, Some("string".to_string()));
    }

    #[test]
    fn test_add_methods() {
        let mut class = Class::new("Animal");
        class.add_method(Member::method("eat").with_visibility(Visibility::Public));
        class.add_method(
            Member::method("digest")
                .with_visibility(Visibility::Protected)
                .with_classifier(Classifier::Abstract),
        );

        assert_eq!(class.methods.len(), 2);
        assert_eq!(class.methods[1].name, "digest");
        assert_eq!(class.methods[1].classifier, Some(Classifier::Abstract));
    }

    #[test]
    fn test_database_add_class() {
        let mut db = ClassDatabase::new();
        db.add_class(Class::new("Animal")).unwrap();
        db.add_class(Class::new("Dog")).unwrap();

        assert_eq!(db.class_count(), 2);
        assert!(db.get_class("Animal").is_some());
        assert!(db.get_class("Cat").is_none());
    }

    #[test]
    fn test_database_add_relationship() {
        let mut db = ClassDatabase::new();
        db.add_class(Class::new("Animal")).unwrap();
        db.add_class(Class::new("Dog")).unwrap();
        db.add_relationship(Relationship::new(
            "Animal",
            "Dog",
            RelationshipKind::Inheritance,
        ))
        .unwrap();

        assert_eq!(db.relationship_count(), 1);
    }

    #[test]
    fn test_visibility_conversion() {
        assert_eq!(Visibility::from_char('+'), Some(Visibility::Public));
        assert_eq!(Visibility::from_char('-'), Some(Visibility::Private));
        assert_eq!(Visibility::from_char('#'), Some(Visibility::Protected));
        assert_eq!(Visibility::from_char('~'), Some(Visibility::Package));
        assert_eq!(Visibility::from_char('x'), None);

        assert_eq!(Visibility::Public.to_char(), '+');
    }

    #[test]
    fn test_classifier_conversion() {
        assert_eq!(Classifier::from_char('*'), Some(Classifier::Abstract));
        assert_eq!(Classifier::from_char('$'), Some(Classifier::Static));
        assert_eq!(Classifier::from_char('x'), None);
    }

    #[test]
    fn test_database_trait_nodes() {
        let mut db = ClassDatabase::new();
        let mut class = Class::new("Person");
        class.add_attribute(Member::attribute("name").with_visibility(Visibility::Public));
        db.add_class(class).unwrap();

        let nodes: Vec<_> = db.nodes().collect();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "Person");
    }

    #[test]
    fn test_get_or_create_class() {
        let mut db = ClassDatabase::new();

        // First call creates
        db.get_or_create_class("Animal");
        assert_eq!(db.class_count(), 1);

        // Second call retrieves existing
        db.get_or_create_class("Animal");
        assert_eq!(db.class_count(), 1);
    }
}
