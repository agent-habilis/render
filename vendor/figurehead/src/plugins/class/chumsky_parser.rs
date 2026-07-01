//! Class diagram parser using chumsky
//!
//! Parses Mermaid.js class diagram syntax into AST structures.

use super::database::{Classifier, RelationshipKind, Visibility};
use crate::core::chumsky_utils::{optional_whitespace, whitespace_required};
use anyhow::Result;
use chumsky::prelude::*;
use chumsky::text::{ident, whitespace};

/// AST types for class diagram parsing

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMember {
    pub visibility: Option<Visibility>,
    pub name: String,
    pub member_type: Option<String>,
    pub classifier: Option<Classifier>,
    pub is_method: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedClass {
    pub name: String,
    pub members: Vec<ParsedMember>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRelationship {
    pub from: String,
    pub to: String,
    pub kind: RelationshipKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Class(ParsedClass),
    Relationship(ParsedRelationship),
}

/// Chumsky-based class diagram parser
pub struct ChumskyClassParser;

impl ChumskyClassParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a complete class diagram
    pub fn parse_diagram(&self, input: &str) -> Result<Vec<Statement>> {
        let parser = Self::diagram_parser();

        parser
            .parse(input)
            .into_result()
            .map_err(|errors| anyhow::anyhow!("Parse errors: {:?}", errors))
    }

    /// Parse a single statement (class or relationship)
    pub fn parse_statement(&self, input: &str) -> Result<Statement> {
        let parser = Self::statement_parser().then_ignore(end());

        parser
            .parse(input)
            .into_result()
            .map_err(|errors| anyhow::anyhow!("Parse errors: {:?}", errors))
    }

    fn diagram_parser<'src>() -> impl Parser<'src, &'src str, Vec<Statement>> {
        // Skip the classDiagram header if present
        let header = text::keyword("classDiagram")
            .or(text::keyword("classdiagram"))
            .or_not();

        // Whitespace that consumes at least one character
        let ws_required = whitespace_required();

        header
            .then_ignore(ws_required.clone().or_not())
            .ignore_then(
                Self::statement_parser()
                    .separated_by(ws_required)
                    .allow_trailing()
                    .collect(),
            )
            .then_ignore(optional_whitespace())
            .then_ignore(end())
    }

    fn statement_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        Self::class_parser()
            .map(Statement::Class)
            .or(Self::relationship_parser().map(Statement::Relationship))
    }

    fn class_parser<'src>() -> impl Parser<'src, &'src str, ParsedClass> + Clone {
        let ws = optional_whitespace();

        // Class name (identifier)
        let class_name = ident().map(|s: &str| s.to_string());

        // Class with body: class Name { members }
        let class_with_body = text::keyword("class")
            .then_ignore(whitespace().at_least(1))
            .ignore_then(class_name)
            .then_ignore(ws.clone())
            .then_ignore(just('{'))
            .then_ignore(ws.clone())
            .then(
                Self::member_parser()
                    .then_ignore(ws.clone())
                    .repeated()
                    .collect(),
            )
            .then_ignore(just('}'))
            .map(|(name, members)| ParsedClass { name, members });

        // Class without body: class Name
        let class_simple = text::keyword("class")
            .then_ignore(whitespace().at_least(1))
            .ignore_then(class_name)
            .map(|name| ParsedClass {
                name,
                members: vec![],
            });

        class_with_body.or(class_simple)
    }

    fn member_parser<'src>() -> impl Parser<'src, &'src str, ParsedMember> + Clone {
        // Visibility prefix: + - # ~
        let visibility = one_of("+-#~").map(Visibility::from_char).or_not();

        // Member name (until : or ( or end of member)
        let member_name = none_of(":(){}*$\n\r")
            .repeated()
            .at_least(1)
            .to_slice()
            .map(|s: &str| s.trim().to_string());

        // Type annotation: : type (stops at } or newline)
        let type_annotation = just(':')
            .ignore_then(none_of("()*${}\n\r").repeated().to_slice())
            .map(|s: &str| s.trim().to_string())
            .or_not();

        // Method signature: (args)
        let method_parens = just('(')
            .ignore_then(none_of(")").repeated())
            .then_ignore(just(')'));

        // Return type for methods: ): type (stops at } or newline)
        let return_type = just(':')
            .ignore_then(none_of("*${}\n\r").repeated().to_slice())
            .map(|s: &str| s.trim().to_string())
            .or_not();

        // Classifier suffix: * or $
        let classifier = just('*')
            .to(Classifier::Abstract)
            .or(just('$').to(Classifier::Static))
            .or_not();

        // Method: visibility name(args): return_type classifier
        let method = visibility
            .then(member_name)
            .then(method_parens)
            .then(return_type)
            .then(classifier)
            .map(|((((vis, name), _args), ret_type), cls)| ParsedMember {
                visibility: vis.flatten(),
                name,
                member_type: ret_type,
                classifier: cls,
                is_method: true,
            });

        // Attribute: visibility name: type classifier
        let attribute = visibility
            .then(member_name)
            .then(type_annotation)
            .then(classifier)
            .map(|(((vis, name), member_type), cls)| ParsedMember {
                visibility: vis.flatten(),
                name,
                member_type,
                classifier: cls,
                is_method: false,
            });

        method.or(attribute)
    }

    fn relationship_parser<'src>() -> impl Parser<'src, &'src str, ParsedRelationship> + Clone {
        // Inline whitespace only (spaces/tabs, NOT newlines)
        // This is critical: we must not consume newlines within a relationship,
        // as they separate statements in the diagram
        let inline_ws = just(' ').or(just('\t')).repeated().ignored();

        // Class name without trailing whitespace
        let class_name = ident().map(|s: &str| s.to_string());

        // Relationship types (longest first to avoid partial matches)
        let rel_kind = just("<|--")
            .to(RelationshipKind::Inheritance)
            .or(just("..|>").to(RelationshipKind::Realization))
            .or(just("*--").to(RelationshipKind::Composition))
            .or(just("o--").to(RelationshipKind::Aggregation))
            .or(just("..>").to(RelationshipKind::Dependency))
            .or(just("-->").to(RelationshipKind::Association))
            .or(just("..").to(RelationshipKind::DashedLink))
            .or(just("--").to(RelationshipKind::Link));

        // Label after colon: : label (consumes to end of line)
        let label = just(':')
            .ignore_then(inline_ws)
            .ignore_then(none_of("\n\r").repeated().to_slice())
            .map(|s: &str| s.trim().to_string())
            .or_not();

        class_name
            .then_ignore(inline_ws)
            .then(rel_kind)
            .then_ignore(inline_ws)
            .then(class_name)
            .then_ignore(inline_ws)
            .then(label)
            .map(|(((from, kind), to), label)| ParsedRelationship {
                from,
                to,
                kind,
                label: label.filter(|s| !s.is_empty()),
            })
    }
}

impl Default for ChumskyClassParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_class() {
        let parser = ChumskyClassParser::new();
        let result = parser.parse_statement("class Animal").unwrap();

        match result {
            Statement::Class(class) => {
                assert_eq!(class.name, "Animal");
                assert!(class.members.is_empty());
            }
            _ => panic!("Expected class statement"),
        }
    }

    #[test]
    fn test_parse_class_with_body() {
        let parser = ChumskyClassParser::new();
        let input = "class Animal { +name: string }";
        let result = parser.parse_statement(input).unwrap();

        match result {
            Statement::Class(class) => {
                assert_eq!(class.name, "Animal");
                assert_eq!(class.members.len(), 1);
                assert_eq!(class.members[0].name, "name");
                assert_eq!(class.members[0].visibility, Some(Visibility::Public));
            }
            _ => panic!("Expected class statement"),
        }
    }

    #[test]
    fn test_parse_inheritance() {
        let parser = ChumskyClassParser::new();
        let result = parser.parse_statement("Animal <|-- Dog").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from, "Animal");
                assert_eq!(rel.to, "Dog");
                assert_eq!(rel.kind, RelationshipKind::Inheritance);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_with_label() {
        let parser = ChumskyClassParser::new();
        let result = parser
            .parse_statement("Customer --> Order : places")
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from, "Customer");
                assert_eq!(rel.to, "Order");
                assert_eq!(rel.kind, RelationshipKind::Association);
                assert_eq!(rel.label, Some("places".to_string()));
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_composition() {
        let parser = ChumskyClassParser::new();
        let result = parser.parse_statement("Person *-- Heart").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.kind, RelationshipKind::Composition);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_aggregation() {
        let parser = ChumskyClassParser::new();
        let result = parser.parse_statement("Library o-- Book").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.kind, RelationshipKind::Aggregation);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_dependency() {
        let parser = ChumskyClassParser::new();
        let result = parser.parse_statement("Client ..> Service").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.kind, RelationshipKind::Dependency);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_realization() {
        let parser = ChumskyClassParser::new();
        let result = parser.parse_statement("Shape ..|> Drawable").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.kind, RelationshipKind::Realization);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_method() {
        let parser = ChumskyClassParser::new();
        let input = "class Animal { +eat() }";
        let result = parser.parse_statement(input).unwrap();

        match result {
            Statement::Class(class) => {
                assert_eq!(class.members.len(), 1);
                assert!(class.members[0].is_method);
                assert_eq!(class.members[0].name, "eat");
            }
            _ => panic!("Expected class statement"),
        }
    }

    #[test]
    fn test_parse_abstract_method() {
        let parser = ChumskyClassParser::new();
        let input = "class Animal { #digest()* }";
        let result = parser.parse_statement(input).unwrap();

        match result {
            Statement::Class(class) => {
                assert_eq!(class.members[0].classifier, Some(Classifier::Abstract));
            }
            _ => panic!("Expected class statement"),
        }
    }

    #[test]
    fn test_parse_static_method() {
        let parser = ChumskyClassParser::new();
        let input = "class Util { +getInstance()$ }";
        let result = parser.parse_statement(input).unwrap();

        match result {
            Statement::Class(class) => {
                assert_eq!(class.members[0].classifier, Some(Classifier::Static));
            }
            _ => panic!("Expected class statement"),
        }
    }

    #[test]
    fn test_parse_full_diagram() {
        let parser = ChumskyClassParser::new();
        let input = r#"classDiagram
            class Animal {
                +name: string
            }
            Animal <|-- Dog"#;

        let result = parser.parse_diagram(input).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_relationships_only() {
        let parser = ChumskyClassParser::new();
        // First test: single relationship without header
        let input1 = "Person *-- Heart";
        let result1 = parser.parse_statement(input1);
        assert!(result1.is_ok(), "Single relationship failed: {:?}", result1);

        // Second test: with header, single relationship
        let input2 = "classDiagram\nPerson *-- Heart";
        let result2 = parser.parse_diagram(input2);
        assert!(
            result2.is_ok(),
            "With header single relationship failed: {:?}",
            result2
        );

        // Third test: full test with indentation
        let input3 = r#"classDiagram
    Person *-- Heart
    Person *-- Brain"#;
        let result3 = parser.parse_diagram(input3);
        assert!(result3.is_ok(), "Full test failed: {:?}", result3);
        assert_eq!(result3.unwrap().len(), 2);
    }
}
