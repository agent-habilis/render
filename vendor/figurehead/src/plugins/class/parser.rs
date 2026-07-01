//! Class diagram parser
//!
//! Parses class diagram syntax into the database using chumsky.

use super::chumsky_parser::{ChumskyClassParser, Statement};
use super::database::{Class, ClassDatabase, Member, Relationship};
use crate::core::Parser;
use anyhow::Result;

/// Class diagram parser using chumsky
pub struct ClassParser {
    chumsky: ChumskyClassParser,
}

impl ClassParser {
    pub fn new() -> Self {
        Self {
            chumsky: ChumskyClassParser::new(),
        }
    }
}

impl Default for ClassParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser<ClassDatabase> for ClassParser {
    fn parse(&self, input: &str, database: &mut ClassDatabase) -> Result<()> {
        let statements = self.chumsky.parse_diagram(input)?;

        for statement in statements {
            match statement {
                Statement::Class(parsed_class) => {
                    let mut class = Class::new(&parsed_class.name);
                    for member in parsed_class.members {
                        let db_member = Member {
                            visibility: member.visibility,
                            name: member.name,
                            member_type: member.member_type,
                            classifier: member.classifier,
                            is_method: member.is_method,
                        };
                        if member.is_method {
                            class.add_method(db_member);
                        } else {
                            class.add_attribute(db_member);
                        }
                    }
                    database.add_class(class)?;
                }
                Statement::Relationship(parsed_rel) => {
                    // Ensure classes exist
                    database.get_or_create_class(&parsed_rel.from);
                    database.get_or_create_class(&parsed_rel.to);

                    let mut rel =
                        Relationship::new(parsed_rel.from, parsed_rel.to, parsed_rel.kind);
                    if let Some(label) = parsed_rel.label {
                        rel = rel.with_label(label);
                    }
                    database.add_relationship(rel)?;
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "class"
    }

    fn version(&self) -> &'static str {
        "0.2.0" // Bumped for chumsky migration
    }

    fn can_parse(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.contains("classdiagram") || lower.contains("class ")
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::{Classifier, RelationshipKind, Visibility};
    use super::*;

    #[test]
    fn test_parse_empty_class() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    class Animal", &mut db)
            .unwrap();

        assert_eq!(db.class_count(), 1);
        assert_eq!(db.classes()[0].name, "Animal");
    }

    #[test]
    fn test_parse_class_with_body() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        let input = r#"classDiagram
    class Animal {
        +name: string
        -age: int
    }"#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.class_count(), 1);
        let class = &db.classes()[0];
        assert_eq!(class.name, "Animal");
        assert_eq!(class.attributes.len(), 2);
        assert_eq!(class.attributes[0].name, "name");
        assert_eq!(class.attributes[0].visibility, Some(Visibility::Public));
        assert_eq!(class.attributes[0].member_type, Some("string".to_string()));
    }

    #[test]
    fn test_parse_methods() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        let input = r#"classDiagram
    class Animal {
        +eat()
        +sleep(): void
        #digest()*
    }"#;

        parser.parse(input, &mut db).unwrap();

        let class = &db.classes()[0];
        assert_eq!(class.methods.len(), 3);
        assert_eq!(class.methods[0].name, "eat");
        assert!(class.methods[0].is_method);
        assert_eq!(class.methods[1].member_type, Some("void".to_string()));
        assert_eq!(class.methods[2].classifier, Some(Classifier::Abstract));
    }

    #[test]
    fn test_parse_all_visibility() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        let input = r#"classDiagram
    class Test {
        +public
        -private
        #protected
        ~package
    }"#;

        parser.parse(input, &mut db).unwrap();

        let attrs = &db.classes()[0].attributes;
        assert_eq!(attrs[0].visibility, Some(Visibility::Public));
        assert_eq!(attrs[1].visibility, Some(Visibility::Private));
        assert_eq!(attrs[2].visibility, Some(Visibility::Protected));
        assert_eq!(attrs[3].visibility, Some(Visibility::Package));
    }

    #[test]
    fn test_parse_static_method() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        let input = r#"classDiagram
    class Util {
        +getInstance()$
    }"#;

        parser.parse(input, &mut db).unwrap();

        let method = &db.classes()[0].methods[0];
        assert_eq!(method.classifier, Some(Classifier::Static));
    }

    #[test]
    fn test_parse_multiple_classes() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        let input = r#"classDiagram
    class Animal {
        +name
    }
    class Dog {
        +breed
    }"#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.class_count(), 2);
        assert_eq!(db.classes()[0].name, "Animal");
        assert_eq!(db.classes()[1].name, "Dog");
    }

    // =========================================================================
    // Relationship parsing tests
    // =========================================================================

    #[test]
    fn test_parse_inheritance() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Animal <|-- Dog", &mut db)
            .unwrap();

        assert_eq!(db.class_count(), 2);
        assert_eq!(db.relationship_count(), 1);
        let rel = &db.relationships()[0];
        assert_eq!(rel.from, "Animal");
        assert_eq!(rel.to, "Dog");
        assert_eq!(rel.kind, RelationshipKind::Inheritance);
    }

    #[test]
    fn test_parse_composition() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Person *-- Heart", &mut db)
            .unwrap();

        assert_eq!(db.relationship_count(), 1);
        let rel = &db.relationships()[0];
        assert_eq!(rel.kind, RelationshipKind::Composition);
    }

    #[test]
    fn test_parse_aggregation() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Library o-- Book", &mut db)
            .unwrap();

        let rel = &db.relationships()[0];
        assert_eq!(rel.kind, RelationshipKind::Aggregation);
    }

    #[test]
    fn test_parse_association() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Student --> Course", &mut db)
            .unwrap();

        let rel = &db.relationships()[0];
        assert_eq!(rel.kind, RelationshipKind::Association);
    }

    #[test]
    fn test_parse_dependency() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Client ..> Service", &mut db)
            .unwrap();

        let rel = &db.relationships()[0];
        assert_eq!(rel.kind, RelationshipKind::Dependency);
    }

    #[test]
    fn test_parse_realization() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Shape ..|> Drawable", &mut db)
            .unwrap();

        let rel = &db.relationships()[0];
        assert_eq!(rel.kind, RelationshipKind::Realization);
    }

    #[test]
    fn test_parse_relationship_with_label() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        parser
            .parse("classDiagram\n    Customer --> Order : places", &mut db)
            .unwrap();

        let rel = &db.relationships()[0];
        assert_eq!(rel.label, Some("places".to_string()));
    }

    #[test]
    fn test_parse_mixed_classes_and_relationships() {
        let parser = ClassParser::new();
        let mut db = ClassDatabase::new();

        let input = r#"classDiagram
    class Animal {
        +name: string
    }
    class Dog {
        +breed: string
    }
    Animal <|-- Dog"#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.class_count(), 2);
        assert_eq!(db.relationship_count(), 1);
    }
}
