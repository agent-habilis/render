//! Flowchart detector implementation
//!
//! Detects flowchart diagram syntax patterns.

use crate::core::Detector;
use tracing::{debug, info, trace};

/// Flowchart detector implementation
pub struct FlowchartDetector;

// Mermaid flowchart connectors we support
const CONNECTORS: [&str; 9] = [
    "-.->", "==>", "===", "-->", "---", "-.-", "--o", "--x", "~~~",
];

impl Default for FlowchartDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowchartDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Detector for FlowchartDetector {
    fn detect(&self, input: &str) -> bool {
        let input = input.trim();
        let input_len = input.len();

        trace!(input_len, "FlowchartDetector::detect called");

        if input.is_empty() {
            debug!("Empty input, detection failed");
            return false;
        }

        // Check for explicit flowchart/graph keywords (highest priority)
        if input.contains("graph") || input.contains("flowchart") {
            info!("Detected flowchart via graph/flowchart keyword");
            return true;
        }

        // Check for arrow patterns
        if CONNECTORS.iter().any(|conn| input.contains(conn)) {
            debug!("Detected flowchart via arrow patterns");
            return true;
        }

        // Check for subgraph syntax
        if input.contains("subgraph") {
            debug!("Detected flowchart via subgraph syntax");
            return true;
        }

        // Check for end keyword (subgraph terminator)
        if input == "end" || input.contains("\nend\n") {
            debug!("Detected flowchart via end keyword");
            return true;
        }

        // Check for node syntax patterns, but be careful to avoid false positives
        // Only detect if there's also arrow-like patterns or it's very specific to Mermaid syntax
        if (input.contains("[") && input.contains("]") && input.contains(" -->"))
            || (input.contains("(") && input.contains(")") && input.contains(" -->"))
            || (input.contains("{") && input.contains("}") && input.contains(" -->"))
        {
            debug!("Detected flowchart via node syntax patterns");
            return true;
        }

        trace!("No flowchart patterns detected");
        false
    }

    fn confidence(&self, input: &str) -> f64 {
        let input = input.trim();

        if input.is_empty() {
            return 0.0;
        }

        let mut score: f64 = 0.0;
        // Primary indicators (high weight)
        if input.contains("graph") {
            score += 0.6;
            // Bonus for proper direction specification
            if input.contains(" TD")
                || input.contains(" LR")
                || input.contains(" TB")
                || input.contains(" BT")
                || input.contains(" RL")
            {
                score += 0.2;
            }
        }
        if input.contains("flowchart") {
            score += 0.6;
            // Bonus for proper direction specification
            if input.contains(" TD")
                || input.contains(" LR")
                || input.contains(" TB")
                || input.contains(" BT")
                || input.contains(" RL")
            {
                score += 0.2;
            }
        }

        // Secondary indicators (medium weight)
        let arrow_count: usize = CONNECTORS
            .iter()
            .map(|conn| input.matches(conn).count())
            .sum();
        if arrow_count > 0 {
            score += 0.15 * (arrow_count as f64).min(3.0); // Cap at 3 arrows
        }

        // Tertiary indicators (lower weight)
        if input.contains("[") && input.contains("]") {
            score += 0.1;
        }
        if input.contains("(") && input.contains(")") {
            score += 0.1;
        }
        if input.contains("{") && input.contains("}") {
            score += 0.1;
        }
        if input.contains("subgraph") {
            score += 0.15;
            if input.contains("end") {
                score += 0.05;
            }
        }

        // Penalty for competing diagram types or function-like syntax
        if input.contains("classDiagram")
            || input.contains("sequenceDiagram")
            || input.contains("gantt")
            || input.contains("pie")
            || input.contains("journey")
            || (input.contains("function") && input.contains("{") && input.contains("}"))
        {
            score = 0.0;
        }

        // Normalize to 0-1 range
        score.min(1.0)
    }

    fn diagram_type(&self) -> &'static str {
        "flowchart"
    }

    fn patterns(&self) -> Vec<&'static str> {
        let mut patterns = vec![
            "graph",
            "flowchart",
            "subgraph",
            "end",
            "[",
            "]",
            "(",
            ")",
            "{",
            "}",
        ];
        patterns.extend(CONNECTORS);
        patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_detector_mermaid_syntax_patterns() {
        let detector = FlowchartDetector::new();

        // Test various Mermaid.js flowchart syntax patterns
        assert!(detector.detect("graph TD"));
        assert!(detector.detect("graph LR"));
        assert!(detector.detect("graph TB"));
        assert!(detector.detect("graph BT"));
        assert!(detector.detect("graph RL"));

        // Test with flowchart keyword
        assert!(detector.detect("flowchart TD"));
        assert!(detector.detect("flowchart LR"));

        // Test with subgraph syntax
        assert!(detector.detect("subgraph Title"));
        assert!(detector.detect("end"));

        // Test arrow patterns
        assert!(detector.detect("A-->B"));
        assert!(detector.detect("A --> B"));
        assert!(detector.detect("A---B"));
        assert!(detector.detect("A --- B"));
        assert!(detector.detect("A --o B"));
        assert!(detector.detect("A --x B"));
        assert!(detector.detect("A -.-> B"));
        assert!(detector.detect("A === B"));
        assert!(detector.detect("A ~~~ B"));

        // Test node syntax patterns (require arrows to avoid false positives)
        assert!(detector.detect("A(Node) --> B"));
        assert!(detector.detect("A[Node] --> B"));
        assert!(detector.detect("A{Node} --> B"));
    }

    #[test]
    fn test_enhanced_detector_confidence_scoring() {
        let detector = FlowchartDetector::new();

        // Graph/flowchart keywords should have highest confidence
        assert!(detector.confidence("graph TD") > 0.5);
        assert!(detector.confidence("flowchart LR") > 0.5);

        // Arrow patterns should have medium confidence
        assert!(detector.confidence("A --> B") > 0.1);
        assert!(detector.confidence("A --- B") > 0.1);
        assert!(detector.confidence("A --o B") > 0.1);

        // Combined patterns should score higher
        let multi_pattern = "graph TD\n    A --> B\n    B --> C";
        assert!(detector.confidence(multi_pattern) > detector.confidence("A --> B"));

        // Non-flowchart content should have zero confidence
        assert_eq!(detector.confidence("This is just regular text"), 0.0);
        assert_eq!(detector.confidence(""), 0.0);
        assert_eq!(detector.confidence("classDiagram"), 0.0);
    }

    #[test]
    fn test_enhanced_detector_rejects_non_flowchart() {
        let detector = FlowchartDetector::new();

        // Should not detect other diagram types
        assert!(!detector.detect("classDiagram"));
        assert!(!detector.detect("sequenceDiagram"));
        assert!(!detector.detect("gantt"));
        assert!(!detector.detect("pie"));
        assert!(!detector.detect("journey"));

        // Should not detect completely unrelated content
        assert!(!detector.detect("This is a story about computers"));
        assert!(!detector.detect("function example() { return true; }"));
        assert!(!detector.detect("123 + 456 = 579"));
    }

    #[test]
    fn test_enhanced_detector_edge_cases() {
        let detector = FlowchartDetector::new();

        // Test with whitespace variations
        assert!(detector.detect("  graph TD  "));
        assert!(detector.detect("\nflowchart LR\n"));
        assert!(detector.detect("A\n-->\nB"));

        // Test case sensitivity (should be case sensitive as per Mermaid.js)
        assert!(detector.detect("graph TD"));
        assert!(!detector.detect("GRAPH TD"));
        assert!(!detector.detect("Graph TD"));
    }

    #[test]
    fn test_enhanced_detector_comprehensive_mermaid_example() {
        let detector = FlowchartDetector::new();
        let mermaid_flowchart = r#"graph TD
    A[Start] --> B{Is it working?}
    B -->|Yes| C[Great!]
    B -->|No| D[Debug]
    C --> E[End]
    D --> F[Fix Issues]
    F --> B

    subgraph "Debug Process"
        F --> G[Check Logs]
        G --> H[Run Tests]
    end"#;

        assert!(detector.detect(mermaid_flowchart));
        assert!(detector.confidence(mermaid_flowchart) > 0.8); // Should be very confident
    }
}
