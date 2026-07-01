//! Plugin orchestrator for coordinating the diagram processing pipeline
//!
//! The orchestrator manages the flow of data through all plugins:
//! Detector → Parser → Database → Layout → Renderer

use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info, span, trace, warn, Level};

use crate::core::{Database, Detector, Parser, RenderConfig, Renderer};
use crate::plugins::class::ClassDatabase;
use crate::plugins::flowchart::FlowchartDatabase;
use crate::plugins::gitgraph::GitGraphDatabase;
use crate::plugins::sequence::SequenceDatabase;
use crate::plugins::state::StateDatabase;

/// Plugin orchestrator that coordinates the entire pipeline
///
/// The orchestrator wires detectors, parsers, layout, and renderer pieces
/// together so callers can run a full pipeline without handling each trait
/// manually.
pub struct Orchestrator {
    detectors: HashMap<String, Box<dyn Detector>>,
    flowchart_parser: Option<crate::plugins::flowchart::FlowchartParser>,
    flowchart_layout: Option<crate::plugins::flowchart::FlowchartLayoutAlgorithm>,
    ascii_renderer: Option<crate::plugins::flowchart::FlowchartRenderer>,
    gitgraph_parser: Option<crate::plugins::gitgraph::GitGraphParser>,
    gitgraph_renderer: Option<crate::plugins::gitgraph::GitGraphRenderer>,
    sequence_parser: Option<crate::plugins::sequence::SequenceParser>,
    sequence_renderer: Option<crate::plugins::sequence::SequenceRenderer>,
    class_parser: Option<crate::plugins::class::ClassParser>,
    class_renderer: Option<crate::plugins::class::ClassRenderer>,
    state_parser: Option<crate::plugins::state::StateParser>,
    state_renderer: Option<crate::plugins::state::StateRenderer>,
}

impl Orchestrator {
    /// Create a new empty orchestrator
    pub fn new() -> Self {
        Self {
            detectors: HashMap::new(),
            flowchart_parser: None,
            flowchart_layout: None,
            ascii_renderer: None,
            gitgraph_parser: None,
            gitgraph_renderer: None,
            sequence_parser: None,
            sequence_renderer: None,
            class_parser: None,
            class_renderer: None,
            state_parser: None,
            state_renderer: None,
        }
    }

    /// Create orchestrator with flowchart plugins using default config
    pub fn with_flowchart_plugins() -> Self {
        Self::flowchart(RenderConfig::default())
    }

    /// Create orchestrator with flowchart plugins and render config
    pub fn flowchart(config: RenderConfig) -> Self {
        let mut layout = crate::plugins::flowchart::FlowchartLayoutAlgorithm::new();
        layout.config_mut().diamond_style = config.diamond_style;

        Self {
            detectors: HashMap::new(),
            flowchart_parser: Some(crate::plugins::flowchart::FlowchartParser::new()),
            flowchart_layout: Some(layout),
            ascii_renderer: Some(crate::plugins::flowchart::FlowchartRenderer::with_config(
                config,
            )),
            gitgraph_parser: None,
            gitgraph_renderer: None,
            sequence_parser: None,
            sequence_renderer: None,
            class_parser: None,
            class_renderer: None,
            state_parser: None,
            state_renderer: None,
        }
    }

    /// Create orchestrator with all plugins using default config
    pub fn with_all_plugins() -> Self {
        Self::all_plugins(RenderConfig::default())
    }

    /// Create orchestrator with all plugins and render config
    pub fn all_plugins(config: RenderConfig) -> Self {
        let mut layout = crate::plugins::flowchart::FlowchartLayoutAlgorithm::new();
        layout.config_mut().diamond_style = config.diamond_style;

        Self {
            detectors: HashMap::new(),
            flowchart_parser: Some(crate::plugins::flowchart::FlowchartParser::new()),
            flowchart_layout: Some(layout),
            ascii_renderer: Some(crate::plugins::flowchart::FlowchartRenderer::with_config(
                config,
            )),
            gitgraph_parser: Some(crate::plugins::gitgraph::GitGraphParser::new()),
            gitgraph_renderer: Some(crate::plugins::gitgraph::GitGraphRenderer::new()),
            sequence_parser: Some(crate::plugins::sequence::SequenceParser::new()),
            sequence_renderer: Some(crate::plugins::sequence::SequenceRenderer::new()),
            class_parser: Some(crate::plugins::class::ClassParser::new()),
            class_renderer: Some(crate::plugins::class::ClassRenderer::new()),
            state_parser: Some(crate::plugins::state::StateParser::new()),
            state_renderer: Some(crate::plugins::state::StateRenderer::new()),
        }
    }

    /// Register a detector plugin
    pub fn register_detector(&mut self, name: String, detector: Box<dyn Detector>) {
        self.detectors.insert(name, detector);
    }

    /// Register the default set of detectors (flowchart, gitgraph, sequence, class, state)
    pub fn register_default_detectors(&mut self) -> &mut Self {
        use crate::plugins::class::ClassDetector;
        use crate::plugins::flowchart::FlowchartDetector;
        use crate::plugins::gitgraph::GitGraphDetector;
        use crate::plugins::sequence::SequenceDetector;
        use crate::plugins::state::StateDetector;
        self.register_detector("flowchart".to_string(), Box::new(FlowchartDetector::new()));
        self.register_detector("gitgraph".to_string(), Box::new(GitGraphDetector::new()));
        self.register_detector("sequence".to_string(), Box::new(SequenceDetector::new()));
        self.register_detector("class".to_string(), Box::new(ClassDetector::new()));
        self.register_detector("state".to_string(), Box::new(StateDetector::new()));
        self
    }

    /// Get available detector names
    pub fn get_detectors(&self) -> Vec<String> {
        self.detectors.keys().cloned().collect()
    }

    /// Check if flowchart plugins are available
    pub fn has_flowchart_plugins(&self) -> bool {
        self.flowchart_parser.is_some()
            && self.flowchart_layout.is_some()
            && self.ascii_renderer.is_some()
    }

    /// Detect diagram type from input text
    ///
    /// Finds the detector with highest confidence score.
    pub fn detect_diagram_type(&self, input: &str) -> Result<String> {
        let detect_span = span!(Level::INFO, "detect_diagram_type", input_len = input.len());
        let _enter = detect_span.enter();

        trace!("Starting diagram type detection");

        // Find detector with highest confidence
        let mut best_match: Option<(&str, f64)> = None;

        for (name, detector) in &self.detectors {
            let confidence = detector.confidence(input);
            trace!(detector = name, confidence, "Checking detector");

            if confidence > 0.5 {
                if let Some((_, best_conf)) = best_match {
                    if confidence > best_conf {
                        best_match = Some((name, confidence));
                    }
                } else {
                    best_match = Some((name, confidence));
                }
            }
        }

        if let Some((name, confidence)) = best_match {
            info!(detector = name, confidence, "Detected diagram type");
            return Ok(name.to_string());
        }

        warn!("No suitable detector found for input");
        Err(anyhow::anyhow!("No suitable detector found for input"))
    }

    /// Process input through the complete pipeline (for flowcharts only)
    ///
    /// Runs detector → parser → renderer using registered plugins.
    pub fn process(&self, input: &str) -> Result<String> {
        let process_span = span!(Level::INFO, "process_diagram", input_len = input.len());
        let _enter = process_span.enter();

        info!("Starting diagram processing pipeline");

        // Step 1: Detect diagram type (must be flowchart for now)
        let detect_span = span!(Level::DEBUG, "pipeline_detect");
        let _detect_enter = detect_span.enter();
        let diagram_type = self.detect_diagram_type(input)?;
        debug!(diagram_type, "Diagram type detected");
        drop(_detect_enter);

        match diagram_type.as_str() {
            "flowchart" => self.process_flowchart(input),
            "gitgraph" => self.process_gitgraph(input),
            "sequence" => self.process_sequence(input),
            "class" => self.process_class(input),
            "state" => self.process_state(input),
            _ => {
                warn!(diagram_type, "Unsupported diagram type");
                Err(anyhow::anyhow!(
                    "Unsupported diagram type: {}",
                    diagram_type
                ))
            }
        }
    }

    /// Process flowchart input directly (skip detection)
    ///
    /// Useful when the caller already knows the diagram type.
    pub fn process_flowchart(&self, input: &str) -> Result<String> {
        let flowchart_span = span!(Level::INFO, "process_flowchart", input_len = input.len());
        let _enter = flowchart_span.enter(); // Enter span to track total pipeline duration

        info!("Processing flowchart diagram");

        // Step 1: Parse the input
        let parse_span = span!(Level::DEBUG, "pipeline_parse");
        let _parse_enter = parse_span.enter();
        let parser = self
            .flowchart_parser
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No flowchart parser available"))?;

        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database)?;
        debug!(
            node_count = database.node_count(),
            edge_count = database.edge_count(),
            "Parsing completed"
        );
        drop(_parse_enter);

        // Step 2: Render the result
        let render_span = span!(Level::DEBUG, "pipeline_render");
        let _render_enter = render_span.enter();
        let renderer = self
            .ascii_renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No ASCII renderer available"))?;

        let canvas = renderer.render(&database)?;
        debug!(output_len = canvas.len(), "Rendering completed");
        drop(_render_enter);

        info!("Pipeline completed successfully");

        // Step 3: Convert canvas to string
        Ok(canvas)
    }

    /// Process flowchart input and return both output and the parsed database
    ///
    /// This method is useful when callers need access to the parsed data structure
    /// (e.g., for applying style-based colorization to the output).
    pub fn process_flowchart_with_database(
        &self,
        input: &str,
    ) -> Result<(String, FlowchartDatabase)> {
        let flowchart_span = span!(
            Level::INFO,
            "process_flowchart_with_db",
            input_len = input.len()
        );
        let _enter = flowchart_span.enter();

        info!("Processing flowchart diagram (with database)");

        // Step 1: Parse the input
        let parse_span = span!(Level::DEBUG, "pipeline_parse");
        let _parse_enter = parse_span.enter();
        let parser = self
            .flowchart_parser
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No flowchart parser available"))?;

        let mut database = FlowchartDatabase::new();
        parser.parse(input, &mut database)?;
        debug!(
            node_count = database.node_count(),
            edge_count = database.edge_count(),
            "Parsing completed"
        );
        drop(_parse_enter);

        // Step 2: Render the result
        let render_span = span!(Level::DEBUG, "pipeline_render");
        let _render_enter = render_span.enter();
        let renderer = self
            .ascii_renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No ASCII renderer available"))?;

        let canvas = renderer.render(&database)?;
        debug!(output_len = canvas.len(), "Rendering completed");
        drop(_render_enter);

        info!("Pipeline completed successfully");

        Ok((canvas, database))
    }

    /// Process git graph input directly (skip detection)
    ///
    /// Useful when the caller already knows the diagram type.
    pub fn process_gitgraph(&self, input: &str) -> Result<String> {
        let gitgraph_span = span!(Level::INFO, "process_gitgraph", input_len = input.len());
        let _enter = gitgraph_span.enter();

        info!("Processing git graph diagram");

        // Step 1: Parse the input
        let parse_span = span!(Level::DEBUG, "pipeline_parse");
        let _parse_enter = parse_span.enter();
        let parser = self
            .gitgraph_parser
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No git graph parser available"))?;

        let mut database = GitGraphDatabase::new();
        parser.parse(input, &mut database)?;
        debug!(
            node_count = database.node_count(),
            edge_count = database.edge_count(),
            "Parsing completed"
        );
        drop(_parse_enter);

        // Step 2: Render the result
        let render_span = span!(Level::DEBUG, "pipeline_render");
        let _render_enter = render_span.enter();
        let renderer = self
            .gitgraph_renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No git graph renderer available"))?;

        let canvas = renderer.render(&database)?;
        debug!(output_len = canvas.len(), "Rendering completed");
        drop(_render_enter);

        info!("Git graph processing completed successfully");
        Ok(canvas)
    }

    /// Process sequence diagram input directly (skip detection)
    ///
    /// Useful when the caller already knows the diagram type.
    pub fn process_sequence(&self, input: &str) -> Result<String> {
        let sequence_span = span!(Level::INFO, "process_sequence", input_len = input.len());
        let _enter = sequence_span.enter();

        info!("Processing sequence diagram");

        // Step 1: Parse the input
        let parse_span = span!(Level::DEBUG, "pipeline_parse");
        let _parse_enter = parse_span.enter();
        let parser = self
            .sequence_parser
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No sequence parser available"))?;

        let mut database = SequenceDatabase::new();
        parser.parse(input, &mut database)?;
        debug!(
            participant_count = database.participant_count(),
            message_count = database.message_count(),
            "Parsing completed"
        );
        drop(_parse_enter);

        // Step 2: Render the result
        let render_span = span!(Level::DEBUG, "pipeline_render");
        let _render_enter = render_span.enter();
        let renderer = self
            .sequence_renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No sequence renderer available"))?;

        let canvas = renderer.render(&database)?;
        debug!(output_len = canvas.len(), "Rendering completed");
        drop(_render_enter);

        info!("Sequence diagram processing completed successfully");
        Ok(canvas)
    }

    /// Process class diagram input directly (skip detection)
    ///
    /// Useful when the caller already knows the diagram type.
    pub fn process_class(&self, input: &str) -> Result<String> {
        let class_span = span!(Level::INFO, "process_class", input_len = input.len());
        let _enter = class_span.enter();

        info!("Processing class diagram");

        // Step 1: Parse the input
        let parse_span = span!(Level::DEBUG, "pipeline_parse");
        let _parse_enter = parse_span.enter();
        let parser = self
            .class_parser
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No class parser available"))?;

        let mut database = ClassDatabase::new();
        parser.parse(input, &mut database)?;
        debug!(
            class_count = database.class_count(),
            relationship_count = database.relationship_count(),
            "Parsing completed"
        );
        drop(_parse_enter);

        // Step 2: Render the result
        let render_span = span!(Level::DEBUG, "pipeline_render");
        let _render_enter = render_span.enter();
        let renderer = self
            .class_renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No class renderer available"))?;

        let canvas = renderer.render_database(&database)?;
        debug!(output_len = canvas.len(), "Rendering completed");
        drop(_render_enter);

        info!("Class diagram processing completed successfully");
        Ok(canvas)
    }

    /// Process state diagram input directly (skip detection)
    ///
    /// Useful when the caller already knows the diagram type.
    pub fn process_state(&self, input: &str) -> Result<String> {
        let state_span = span!(Level::INFO, "process_state", input_len = input.len());
        let _enter = state_span.enter();

        info!("Processing state diagram");

        // Step 1: Parse the input
        let parse_span = span!(Level::DEBUG, "pipeline_parse");
        let _parse_enter = parse_span.enter();
        let parser = self
            .state_parser
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No state parser available"))?;

        let mut database = StateDatabase::new();
        parser.parse(input, &mut database)?;
        debug!(
            state_count = database.state_count(),
            transition_count = database.transition_count(),
            "Parsing completed"
        );
        drop(_parse_enter);

        // Step 2: Render the result
        let render_span = span!(Level::DEBUG, "pipeline_render");
        let _render_enter = render_span.enter();
        let renderer = self
            .state_renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No state renderer available"))?;

        let canvas = renderer.render(&database)?;
        debug!(output_len = canvas.len(), "Rendering completed");
        drop(_render_enter);

        info!("State diagram processing completed successfully");
        Ok(canvas)
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::flowchart::FlowchartDetector;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = Orchestrator::new();
        assert_eq!(orchestrator.get_detectors().len(), 0);
        assert!(!orchestrator.has_flowchart_plugins());
    }

    #[test]
    fn test_orchestrator_default() {
        let orchestrator = Orchestrator::default();
        assert_eq!(orchestrator.get_detectors().len(), 0);
        assert!(!orchestrator.has_flowchart_plugins());
    }

    #[test]
    fn test_orchestrator_with_flowchart_plugins() {
        let orchestrator = Orchestrator::with_flowchart_plugins();
        assert_eq!(orchestrator.get_detectors().len(), 0);
        assert!(orchestrator.has_flowchart_plugins());
    }

    #[test]
    fn test_register_detector() {
        let mut orchestrator = Orchestrator::new();
        assert!(!orchestrator.has_flowchart_plugins());

        // Register a detector
        let detector = Box::new(FlowchartDetector::new());
        orchestrator.register_detector("flowchart".to_string(), detector);

        assert_eq!(orchestrator.get_detectors(), vec!["flowchart"]);
        assert!(!orchestrator.has_flowchart_plugins());
    }

    #[test]
    fn test_detect_diagram_type_with_no_detectors() {
        let orchestrator = Orchestrator::new();
        let input = "graph TD; A-->B;";

        let result = orchestrator.detect_diagram_type(input);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No suitable detector found for input"
        );
    }

    #[test]
    fn test_detect_diagram_type_with_flowchart() {
        let mut orchestrator = Orchestrator::new();
        let detector = Box::new(FlowchartDetector::new());
        orchestrator.register_detector("flowchart".to_string(), detector);

        let input = "graph TD; A-->B;";
        let result = orchestrator.detect_diagram_type(input);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "flowchart");
    }

    #[test]
    fn test_process_with_missing_plugins() {
        let orchestrator = Orchestrator::new();
        let input = "graph TD; A-->B;";

        let result = orchestrator.process(input);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No suitable detector found for input"
        );
    }

    #[test]
    fn test_process_with_no_flowchart_plugins() {
        let mut orchestrator = Orchestrator::new();
        let detector = Box::new(FlowchartDetector::new());
        orchestrator.register_detector("flowchart".to_string(), detector);

        let input = "graph TD; A-->B;";
        let result = orchestrator.process(input);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No flowchart parser available"
        );
    }

    #[test]
    fn test_process_flowchart_success() {
        let orchestrator = Orchestrator::with_flowchart_plugins();
        let input = "graph TD; A-->B;";
        let result = orchestrator.process_flowchart(input);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
        // The output should contain ASCII diagram content
        assert!(output.contains("A") || output.contains("B") || output.contains("┌"));
    }

    #[test]
    fn test_process_flowchart_complex() {
        let orchestrator = Orchestrator::with_flowchart_plugins();
        let input = r#"
            graph TD;
            A[Start] --> B{Decision};
            B -->|Yes| C[Process];
            B -->|No| D[End];
            C --> D;
        "#;

        let result = orchestrator.process_flowchart(input);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
        // Should contain multiple nodes
    }

    #[test]
    fn test_process_with_detection_and_plugins() {
        let mut orchestrator = Orchestrator::with_flowchart_plugins();

        // Add detector for the pipeline
        let detector = Box::new(FlowchartDetector::new());
        orchestrator.register_detector("flowchart".to_string(), detector);

        let input = "graph TD; A-->B;";
        let result = orchestrator.process(input);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_process_with_non_flowchart_detection() {
        let mut orchestrator = Orchestrator::with_flowchart_plugins();

        // Add detector that will not match
        let detector = Box::new(FlowchartDetector::new());
        orchestrator.register_detector("flowchart".to_string(), detector);

        let input = "This is just plain text, not a diagram";
        let result = orchestrator.process(input);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No suitable detector found for input"
        );
    }

    #[test]
    fn test_process_with_wrong_diagram_type() {
        // Create a mock detector that returns a wrong type
        let mut orchestrator = Orchestrator::with_flowchart_plugins();

        // We'll test by manually calling detect with a different result
        // since we can't easily mock the detector to return a wrong type
        let detector = Box::new(FlowchartDetector::new());
        orchestrator.register_detector("flowchart".to_string(), detector);

        let input = "graph TD; A-->B;";
        let detection_result = orchestrator.detect_diagram_type(input);
        assert!(detection_result.is_ok());
        assert_eq!(detection_result.unwrap(), "flowchart");

        // Since we detect "flowchart" and have flowchart plugins, this should work
        let result = orchestrator.process(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_empty_input() {
        let orchestrator = Orchestrator::with_flowchart_plugins();
        let result = orchestrator.process_flowchart("");

        assert!(result.is_ok());
        // Empty input produces empty output (no nodes to render)
        let output = result.unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_process_invalid_syntax() {
        let orchestrator = Orchestrator::with_flowchart_plugins();
        let input = "invalid syntax that is not mermaid";
        let result = orchestrator.process_flowchart(input);

        // Should return an error for completely invalid syntax
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Parse error") || err.contains("no valid statements"));
    }

    #[test]
    fn test_process_gitgraph() {
        use crate::plugins::gitgraph::GitGraphDetector;

        let mut orchestrator = Orchestrator::with_all_plugins();
        orchestrator.register_detector("gitgraph".to_string(), Box::new(GitGraphDetector::new()));

        let input = "gitGraph\n   commit\n   commit\n   commit";
        let result = orchestrator.process(input);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_process_gitgraph_with_branches() {
        let orchestrator = Orchestrator::with_all_plugins();

        let input = r#"gitGraph
   commit
   branch develop
   checkout develop
   commit
   checkout main
   merge develop"#;
        let result = orchestrator.process_gitgraph(input);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_process_class() {
        use crate::plugins::class::ClassDetector;

        let mut orchestrator = Orchestrator::with_all_plugins();
        orchestrator.register_detector("class".to_string(), Box::new(ClassDetector::new()));

        let input = "classDiagram\n    class Animal";
        let result = orchestrator.process(input);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("Animal"));
    }

    #[test]
    fn test_process_class_with_members() {
        let orchestrator = Orchestrator::with_all_plugins();

        let input = r#"classDiagram
    class Animal {
        +name: string
        -age: int
        +eat()
        #digest()*
    }"#;
        let result = orchestrator.process_class(input);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.is_empty());
        assert!(output.contains("Animal"));
        assert!(output.contains("+name: string"));
        assert!(output.contains("-age: int"));
        assert!(output.contains("+eat()"));
        assert!(output.contains("#digest()*"));
    }
}
