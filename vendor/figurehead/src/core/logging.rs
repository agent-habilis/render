//! Logging infrastructure for diagram processing
//!
//! This module provides structured logging using the `tracing` crate.
//! It supports configurable log levels and formats, and is designed to be
//! WASM-compatible for future browser support.
//!
//! # Usage
//!
//! ```rust
//! use figurehead::core::logging::init_logging;
//!
//! // Initialize with default settings (may fail if already initialized)
//! let _ = init_logging(None, None);
//!
//! // Or with custom level and format
//! let _ = init_logging(Some("debug"), Some("pretty"));
//! ```
//!
//! # Log Levels
//!
//! - `trace`: Very detailed information, typically only interesting when debugging
//! - `debug`: Detailed information for debugging
//! - `info`: General informational messages (default)
//! - `warn`: Warning messages
//! - `error`: Error messages
//!
//! # Log Formats
//!
//! - `compact`: Single-line format, good for production
//! - `pretty`: Multi-line format with colors, good for development
//! - `json`: JSON format, good for log aggregation systems
//!
//! # Environment Variables
//!
//! Logging can be configured via environment variables:
//! - `FIGUREHEAD_LOG_LEVEL`: Set log level (trace|debug|info|warn|error)
//! - `RUST_LOG`: Alternative way to set log level (tracing-subscriber standard)
//!
//! # WASM Compatibility
//!
//! The logging infrastructure is designed to work in WASM environments.
//! For WASM builds, use `tracing-wasm` instead of `tracing-subscriber`.
//!
//! # Adding Tracing to Custom Plugins
//!
//! When implementing a new diagram type plugin, add tracing spans and events
//! to provide visibility into the processing pipeline:
//!
//! ```rust
//! use tracing::{debug, info, span, trace, Level};
//!
//! // Example: Adding tracing to a parser implementation
//! fn parse_with_tracing(input: &str) -> Result<(), Box<dyn std::error::Error>> {
//!     let parse_span = span!(Level::INFO, "parse_mydiagram", input_len = input.len());
//!     let _enter = parse_span.enter();
//!
//!     trace!("Starting parsing");
//!
//!     // Parse stages
//!     let stage_span = span!(Level::DEBUG, "parse_stage");
//!     let _stage_enter = stage_span.enter();
//!     // ... parsing logic ...
//!     debug!(input_len = input.len(), "Parsed input");
//!     drop(_stage_enter);
//!
//!     info!("Parsing completed");
//!     Ok(())
//! }
//!
//! // Note: This example doesn't actually parse anything, just demonstrates tracing usage
//! # parse_with_tracing("example input").unwrap();
//! ```
//!
//! # Filtering Logs
//!
//! You can filter logs by component using the log level syntax:
//!
//! ```bash
//! # Show only parser logs at debug level
//! RUST_LOG="figurehead::plugins::flowchart::parser=debug" figurehead convert input.mmd
//!
//! # Show all logs at info level, but layout at trace level
//! RUST_LOG="info,figurehead::plugins::flowchart::layout=trace" figurehead convert input.mmd
//! ```

use std::str::FromStr;

#[cfg(not(target_arch = "wasm32"))]
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};

#[cfg(target_arch = "wasm32")]
use tracing_wasm::WASMLayerConfig;

/// Log format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Compact single-line format
    Compact,
    /// Pretty multi-line format with colors
    Pretty,
    /// JSON format for log aggregation
    Json,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "compact" => Ok(LogFormat::Compact),
            "pretty" => Ok(LogFormat::Pretty),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("Unknown log format: {}", s)),
        }
    }
}

impl LogFormat {
    /// Get all valid format names
    pub fn variants() -> &'static [&'static str] {
        &["compact", "pretty", "json"]
    }
}

/// Initialize the tracing subscriber with the given log level and format
///
/// # Arguments
///
/// * `level` - Optional log level string (trace|debug|info|warn|error).
///   If None, uses environment variable `FIGUREHEAD_LOG_LEVEL` or `RUST_LOG`,
///   or defaults to `info`.
/// * `format` - Optional log format (compact|pretty|json).
///   If None, uses environment variable `FIGUREHEAD_LOG_FORMAT`,
///   or defaults to `compact`.
///
/// # Returns
///
/// Returns an error if initialization fails (e.g., subscriber already initialized).
///
/// # Example
///
/// ```rust
/// use figurehead::core::logging::init_logging;
///
/// // Initialize with defaults (may fail if already initialized)
/// let _ = init_logging(None, None);
///
/// // Initialize with custom settings
/// let _ = init_logging(Some("debug"), Some("pretty"));
/// ```
pub fn init_logging(
    level: Option<&str>,
    format: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        // WASM builds use tracing-wasm which logs to browser console
        // tracing-wasm v0.1.0 doesn't support level filtering in config,
        // but we can use EnvFilter if needed. For now, use default config.
        // Log level filtering can be done via RUST_LOG env var in browser console.
        // Format parameter is ignored in WASM (always logs to console)
        let _ = format; // Suppress unused warning
        tracing_wasm::set_as_global_default_with_config(WASMLayerConfig::default());

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native builds use tracing-subscriber
        // Determine log level from parameter, env var, or default
        let log_level = level
            .map(|s| s.to_string())
            .or_else(|| std::env::var("FIGUREHEAD_LOG_LEVEL").ok())
            .or_else(|| std::env::var("RUST_LOG").ok())
            .unwrap_or_else(|| "info".to_string());

        // Determine format from parameter, env var, or default
        let log_format = format
            .map(|s| s.to_string())
            .or_else(|| std::env::var("FIGUREHEAD_LOG_FORMAT").ok())
            .unwrap_or_else(|| "compact".to_string());

        // Parse log level
        let filter = if log_level == "off" {
            EnvFilter::new("off")
        } else {
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(&log_level))
                .unwrap_or_else(|_| EnvFilter::new("info"))
        };

        // Parse format
        let format =
            LogFormat::from_str(&log_format).map_err(|e| format!("Invalid log format: {}", e))?;

        // Build subscriber based on format
        match format {
            LogFormat::Compact => {
                Registry::default()
                    .with(filter)
                    .with(
                        fmt::Layer::default()
                            .with_target(false)
                            .with_level(true)
                            .with_file(false)
                            .with_line_number(false)
                            .with_span_events(FmtSpan::NONE),
                    )
                    .try_init()?;
            }
            LogFormat::Pretty => {
                Registry::default()
                    .with(filter)
                    .with(
                        fmt::Layer::default()
                            .with_target(true)
                            .with_level(true)
                            .with_file(true)
                            .with_line_number(true)
                            .with_span_events(FmtSpan::ACTIVE)
                            .pretty(),
                    )
                    .try_init()?;
            }
            LogFormat::Json => {
                Registry::default()
                    .with(filter)
                    .with(
                        fmt::Layer::default()
                            .with_target(true)
                            .with_level(true)
                            .with_file(true)
                            .with_line_number(true)
                            .with_span_events(FmtSpan::ACTIVE)
                            .json(),
                    )
                    .try_init()?;
            }
        }

        Ok(())
    }
}

/// Initialize logging with default settings (info level, compact format)
///
/// This is a convenience function that calls `init_logging(None, None)`.
pub fn init_default_logging() -> Result<(), Box<dyn std::error::Error>> {
    init_logging(None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_parsing() {
        assert_eq!(LogFormat::from_str("compact").unwrap(), LogFormat::Compact);
        assert_eq!(LogFormat::from_str("pretty").unwrap(), LogFormat::Pretty);
        assert_eq!(LogFormat::from_str("json").unwrap(), LogFormat::Json);
        assert_eq!(LogFormat::from_str("COMPACT").unwrap(), LogFormat::Compact);
        assert!(LogFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_log_format_variants() {
        let variants = LogFormat::variants();
        assert!(variants.contains(&"compact"));
        assert!(variants.contains(&"pretty"));
        assert!(variants.contains(&"json"));
    }
}
