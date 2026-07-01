//! Core error types for diagram processing
//!
//! This module defines common error types used throughout the diagram processing pipeline.

use thiserror::Error;

/// Core error types for diagram processing
#[derive(Error, Debug)]
pub enum DiagramError {
    #[error("Parse error: {message} at line {line}, column {column}")]
    ParseError {
        message: String,
        line: usize,
        column: usize,
    },

    #[error("Layout error: {message}")]
    LayoutError { message: String },

    #[error("Render error: {message}")]
    RenderError { message: String },

    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("Detection error: {message}")]
    DetectionError { message: String },

    #[error("IO error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    #[error("Unknown diagram type: {diagram_type}")]
    UnknownDiagramType { diagram_type: String },
}

impl DiagramError {
    /// Create a new parse error
    pub fn parse_error(message: String, line: usize, column: usize) -> Self {
        Self::ParseError {
            message,
            line,
            column,
        }
    }

    /// Create a new layout error
    pub fn layout_error(message: String) -> Self {
        Self::LayoutError { message }
    }

    /// Create a new render error
    pub fn render_error(message: String) -> Self {
        Self::RenderError { message }
    }

    /// Create a new database error
    pub fn database_error(message: String) -> Self {
        Self::DatabaseError { message }
    }

    /// Create a new detection error
    pub fn detection_error(message: String) -> Self {
        Self::DetectionError { message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let error = DiagramError::parse_error("Invalid syntax".to_string(), 5, 10);
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("Parse error"));
        assert!(error_msg.contains("Invalid syntax"));
        assert!(error_msg.contains("line 5"));
        assert!(error_msg.contains("column 10"));
    }

    #[test]
    fn test_layout_error() {
        let error = DiagramError::layout_error("Layout failed".to_string());
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("Layout error"));
        assert!(error_msg.contains("Layout failed"));
    }

    #[test]
    fn test_render_error() {
        let error = DiagramError::render_error("Render failed".to_string());
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("Render error"));
        assert!(error_msg.contains("Render failed"));
    }

    #[test]
    fn test_database_error() {
        let error = DiagramError::database_error("Database error".to_string());
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("Database error"));
    }

    #[test]
    fn test_detection_error() {
        let error = DiagramError::detection_error("Detection failed".to_string());
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("Detection error"));
        assert!(error_msg.contains("Detection failed"));
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let error: DiagramError = io_err.into();
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("IO error"));
        assert!(error_msg.contains("File not found"));
    }
}
