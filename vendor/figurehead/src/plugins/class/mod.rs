//! Class diagram plugin
//!
//! Implements class diagram parsing and rendering.

mod chumsky_parser;
mod database;
mod detector;
mod layout;
mod parser;
mod renderer;

pub use chumsky_parser::ChumskyClassParser;
pub use database::{
    Class, ClassDatabase, Classifier, Member, Relationship, RelationshipKind, Visibility,
};
pub use detector::ClassDetector;
pub use layout::{
    ClassLayoutAlgorithm, ClassLayoutResult, PositionedClass, PositionedRelationship,
};
pub use parser::ClassParser;
pub use renderer::ClassRenderer;
