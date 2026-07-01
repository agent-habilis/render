//! WebAssembly bindings for Figurehead
//!
//! This module provides WASM-compatible wrappers for diagram processing.
//! It uses conditional compilation to provide browser-friendly APIs.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::core::{CharacterSet, Database, Parser, RenderConfig, Renderer};
#[cfg(target_arch = "wasm32")]
use crate::plugins::flowchart::{
    clear_warnings, take_warnings, FlowchartDatabase, FlowchartParser, FlowchartRenderer,
};
#[cfg(target_arch = "wasm32")]
use crate::plugins::Orchestrator;

#[cfg(target_arch = "wasm32")]
use console_error_panic_hook;

/// Initialize WASM module
///
/// Sets up panic hooks and logging for better error messages in the browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error messages
    console_error_panic_hook::set_once();

    // Initialize logging for WASM (logs to browser console)
    use crate::core::logging::init_logging;
    let _ = init_logging(Some("info"), None);
}

/// Render a Mermaid flowchart diagram to ASCII art
///
/// # Arguments
/// * `input` - Mermaid flowchart syntax (e.g., "graph LR; A-->B")
///
/// # Returns
/// * The ASCII art representation as a String
/// * Throws a JavaScript error if parsing or rendering fails
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_flowchart(input: &str) -> String {
    let parser = FlowchartParser::new();
    let mut database = FlowchartDatabase::new();

    parser
        .parse(input, &mut database)
        .map_err(|e| format!("Parse error: {}", e))
        .expect("Failed to parse diagram");

    let renderer = FlowchartRenderer::new();
    renderer
        .render(&database)
        .map_err(|e| format!("Render error: {}", e))
        .expect("Failed to render diagram")
}

/// Render a Mermaid flowchart diagram with a specific character set
///
/// # Arguments
/// * `input` - Mermaid flowchart syntax
/// * `style` - Character set style ("ascii", "unicode", "unicode-math", or "compact")
///
/// # Returns
/// * The ASCII art representation as a String
/// * Throws a JavaScript error if parsing or rendering fails
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_flowchart_with_style(input: &str, style: &str) -> String {
    let character_set: CharacterSet = style.parse().unwrap_or_else(|e| panic!("{}", e));

    let parser = FlowchartParser::new();
    let mut database = FlowchartDatabase::new();

    parser
        .parse(input, &mut database)
        .map_err(|e| format!("Parse error: {}", e))
        .expect("Failed to parse diagram");

    let renderer = FlowchartRenderer::with_style(character_set);
    renderer
        .render(&database)
        .map_err(|e| format!("Render error: {}", e))
        .expect("Failed to render diagram")
}

/// Parse a Mermaid flowchart and return node/edge counts
///
/// # Arguments
/// * `input` - Mermaid flowchart syntax
///
/// # Returns
/// * JSON string with node_count, edge_count, and direction
/// * Throws a JavaScript error if parsing fails
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn parse_flowchart(input: &str) -> String {
    let parser = FlowchartParser::new();
    let mut database = FlowchartDatabase::new();

    parser
        .parse(input, &mut database)
        .map_err(|e| format!("Parse error: {}", e))
        .expect("Failed to parse diagram");

    let result = serde_json::json!({
        "node_count": database.node_count(),
        "edge_count": database.edge_count(),
        "direction": format!("{:?}", database.direction()),
    });

    serde_json::to_string(&result).expect("Failed to serialize JSON")
}

/// Render any supported diagram type (auto-detects)
///
/// # Arguments
/// * `input` - Mermaid diagram syntax (flowchart, gitgraph, etc.)
///
/// # Returns
/// * The ASCII art representation as a String
/// * Throws a JavaScript error if parsing or rendering fails
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_diagram(input: &str) -> Result<String, JsValue> {
    let mut orchestrator = Orchestrator::with_all_plugins();
    orchestrator.register_default_detectors();
    orchestrator
        .process(input)
        .map_err(|e| JsValue::from_str(&format!("{}", e)))
}

/// Render any supported diagram type with a specific style (auto-detects)
///
/// # Arguments
/// * `input` - Mermaid diagram syntax (flowchart, gitgraph, etc.)
/// * `style` - Character set style ("ascii", "unicode", "unicode-math", or "compact")
///
/// # Returns
/// * The ASCII art representation as a String
/// * Throws a JavaScript error if parsing or rendering fails
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_diagram_with_style(input: &str, style: &str) -> Result<String, JsValue> {
    let character_set: CharacterSet = style.parse().map_err(|e| JsValue::from_str(&e))?;

    let config = RenderConfig {
        style: character_set,
        ..Default::default()
    };
    let mut orchestrator = Orchestrator::all_plugins(config);
    orchestrator.register_default_detectors();
    orchestrator
        .process(input)
        .map_err(|e| JsValue::from_str(&format!("{}", e)))
}

/// Render a diagram and return JSON with output and any warnings
///
/// # Arguments
/// * `input` - Mermaid diagram syntax (flowchart, gitgraph, etc.)
/// * `style` - Character set style ("ascii", "unicode", "unicode-math", or "compact")
///
/// # Returns
/// * JSON string with fields: output, warnings, error
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_diagram_json(input: &str, style: &str) -> String {
    clear_warnings();

    let character_set: CharacterSet = match style.parse() {
        Ok(cs) => cs,
        Err(e) => {
            return serde_json::json!({
                "output": "",
                "warnings": [],
                "error": e
            })
            .to_string();
        }
    };

    let config = RenderConfig {
        style: character_set,
        ..Default::default()
    };
    let mut orchestrator = Orchestrator::all_plugins(config);
    orchestrator.register_default_detectors();

    match orchestrator.process(input) {
        Ok(output) => {
            let warnings = take_warnings();
            serde_json::json!({
                "output": output,
                "warnings": warnings,
                "error": null
            })
            .to_string()
        }
        Err(e) => {
            let warnings = take_warnings();
            serde_json::json!({
                "output": "",
                "warnings": warnings,
                "error": format!("{}", e)
            })
            .to_string()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod wasm {
    //! Placeholder module for non-WASM builds
    //!
    //! This module is only available when compiling for WASM targets.
}
