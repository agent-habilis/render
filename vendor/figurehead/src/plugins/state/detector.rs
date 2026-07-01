//! State diagram detector
//!
//! Identifies state diagram syntax from input text.

use crate::core::Detector;

/// Detector for state diagram syntax
pub struct StateDetector;

impl StateDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StateDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for StateDetector {
    fn detect(&self, input: &str) -> bool {
        self.confidence(input) > 0.5
    }

    fn confidence(&self, input: &str) -> f64 {
        let input_lower = input.to_lowercase();
        let trimmed = input_lower.trim();

        // Must start with "statediagram" or "statediagram-v2"
        if trimmed.starts_with("statediagram") {
            return 1.0;
        }

        // Check for state-specific patterns
        let has_terminal = input.contains("[*]");
        let has_state_keyword = input_lower.contains("state ");
        let has_transition = input.contains("-->");

        if has_terminal && has_transition {
            return 0.8;
        }

        if has_state_keyword && has_transition {
            return 0.7;
        }

        if has_terminal {
            return 0.5;
        }

        0.0
    }

    fn diagram_type(&self) -> &'static str {
        "state"
    }

    fn patterns(&self) -> Vec<&'static str> {
        vec!["stateDiagram", "stateDiagram-v2", "[*]", "-->", "state "]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_state_diagram_keyword() {
        let detector = StateDetector::new();
        assert!(detector.detect("stateDiagram\n    [*] --> Idle"));
        assert!(detector.detect("stateDiagram-v2\n    [*] --> Idle"));
        assert!(detector.detect("STATEDIAGRAM\n    [*] --> Idle"));
    }

    #[test]
    fn test_detects_terminal_states() {
        let detector = StateDetector::new();
        assert!(detector.detect("[*] --> Idle\nIdle --> [*]"));
    }

    #[test]
    fn test_confidence_scoring() {
        let detector = StateDetector::new();

        // Full keyword = 1.0
        assert_eq!(
            detector.confidence("stateDiagram-v2\n    [*] --> Idle"),
            1.0
        );

        // Terminal + transition = 0.8
        assert!(detector.confidence("[*] --> Idle") >= 0.8);

        // state keyword + transition = 0.7
        assert!(detector.confidence("state \"desc\" as s1\ns1 --> s2") >= 0.7);

        // Just terminal = 0.5
        assert!(detector.confidence("[*]") >= 0.5);

        // Nothing = 0.0
        assert_eq!(detector.confidence("graph TD; A-->B"), 0.0);
    }

    #[test]
    fn test_rejects_flowchart() {
        let detector = StateDetector::new();
        // Flowchart uses --> but no [*] or state keyword
        assert!(!detector.detect("graph TD; A-->B-->C"));
        assert!(!detector.detect("flowchart LR; A-->B"));
    }

    #[test]
    fn test_rejects_sequence() {
        let detector = StateDetector::new();
        assert!(!detector.detect("sequenceDiagram\n    Alice->>Bob: Hello"));
    }
}
