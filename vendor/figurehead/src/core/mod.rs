//! Core abstractions for diagram processing
//!
//! This module defines the fundamental traits that all diagram types must implement,
//! following the mermaid.js architecture with SOLID principles.

mod box_drawing;
mod canvas;
pub mod chumsky_utils;
mod database;
mod detector;
mod diagram;
mod edge_routing;
mod error;
mod layout;
pub mod logging;
mod parser;
mod renderer;
mod syntax;
mod text;
mod types;

pub use box_drawing::*;
pub use canvas::*;
pub use chumsky_utils::*;
pub use database::*;
pub use detector::*;
pub use diagram::*;
pub use edge_routing::*;
pub use error::*;
pub use layout::*;
pub use logging::*;
pub use parser::*;
pub use renderer::*;
pub use syntax::*;
pub use text::*;
pub use types::*;
