//! Core detector trait for diagram type identification
//!
//! This trait defines the interface for detecting diagram types
//! from markup language patterns.

/// Core trait for diagram type detectors
///
/// This trait represents the detection layer that identifies diagram types
/// from markup patterns. Each diagram type provides a detector that can
/// recognize its specific syntax patterns.
///
/// # Example
/// ```
/// use figurehead::core::Detector;
/// use figurehead::plugins::flowchart::FlowchartDetector;
///
/// let detector = FlowchartDetector::new();
/// let is_flowchart = detector.detect("graph TD\n    A --> B");
/// ```
pub trait Detector: Send + Sync {
    /// Detect if the input matches this diagram type
    fn detect(&self, input: &str) -> bool;

    /// Get the confidence level of the detection (0.0 to 1.0)
    fn confidence(&self, input: &str) -> f64;

    /// Get the diagram type name
    fn diagram_type(&self) -> &'static str;

    /// Get key patterns that this detector looks for
    fn patterns(&self) -> Vec<&'static str>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::flowchart::*;

    #[test]
    fn test_diagram_detector_trait_exists() {
        // This test ensures we have the DiagramDetector trait
        let detector = FlowchartDetector::new();
        assert_eq!(detector.diagram_type(), "flowchart");
        let patterns = detector.patterns();
        let required = [
            "graph",
            "flowchart",
            "-->",
            "---",
            "subgraph",
            "end",
            "[",
            "]",
            "(",
            ")",
            "{",
            "}",
            "-.->",
            "==>",
            "===",
            "-.-",
            "--o",
            "--x",
            "~~~",
        ];
        for pat in required {
            assert!(
                patterns.contains(&pat),
                "expected pattern {} to be advertised",
                pat
            );
        }
    }

    #[test]
    fn test_detector_patterns() {
        let detector = FlowchartDetector::new();

        // Test basic detection
        assert!(detector.detect("graph TD"));
        assert!(detector.detect("flowchart LR"));
        assert!(detector.detect("A --> B"));
        assert!(detector.detect("A --- B"));

        // Test confidence scoring
        let graph_confidence = detector.confidence("graph TD");
        let flowchart_confidence = detector.confidence("flowchart LR");
        let arrow_confidence = detector.confidence("A --> B");

        println!("graph confidence: {}", graph_confidence);
        println!("flowchart confidence: {}", flowchart_confidence);
        println!("arrow confidence: {}", arrow_confidence);

        // These should be > 0 based on our scoring system
        assert!(graph_confidence > 0.0);
        assert!(flowchart_confidence > 0.0);
        assert!(arrow_confidence > 0.0);
        assert_eq!(detector.confidence("random text"), 0.0);
    }
}
