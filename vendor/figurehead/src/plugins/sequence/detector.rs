//! Sequence diagram detector
//!
//! Identifies sequence diagram syntax from input text.

use crate::core::Detector;

/// Detector for sequence diagram syntax
pub struct SequenceDetector;

impl SequenceDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SequenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for SequenceDetector {
    fn detect(&self, input: &str) -> bool {
        self.confidence(input) > 0.5
    }

    fn confidence(&self, input: &str) -> f64 {
        let input_lower = input.to_lowercase();
        let trimmed = input_lower.trim();

        // Must start with "sequencediagram"
        if trimmed.starts_with("sequencediagram") {
            return 1.0;
        }

        // Check for sequence-specific patterns
        let has_sequence_arrows = input.contains("->>")
            || input.contains("-->>")
            || input.contains("-)")
            || input.contains("--)")
            || input.contains("-x")
            || input.contains("--x");

        let has_participant =
            input_lower.contains("participant ") || input_lower.contains("actor ");

        if has_sequence_arrows && has_participant {
            return 0.8;
        }

        if has_sequence_arrows {
            return 0.6;
        }

        0.0
    }

    fn diagram_type(&self) -> &'static str {
        "sequence"
    }

    fn patterns(&self) -> Vec<&'static str> {
        vec![
            "sequenceDiagram",
            "->>",
            "-->>",
            "->",
            "-->",
            "-)",
            "--)",
            "participant",
            "actor",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_sequence_diagram_keyword() {
        let detector = SequenceDetector::new();
        assert!(detector.detect("sequenceDiagram\n    Alice->>Bob: Hello"));
        assert!(detector.detect("sequencediagram\n    Alice->>Bob: Hello"));
        assert!(detector.detect("SEQUENCEDIAGRAM\n    Alice->>Bob: Hello"));
    }

    #[test]
    fn test_detects_sequence_arrows() {
        let detector = SequenceDetector::new();
        // With arrows only - lower confidence but still detects
        assert!(detector.detect("Alice->>Bob: Hello"));
        assert!(detector.detect("Alice-->>Bob: Response"));
    }

    #[test]
    fn test_confidence_scoring() {
        let detector = SequenceDetector::new();

        // Full keyword = 1.0
        assert_eq!(detector.confidence("sequenceDiagram\n    A->>B: Hi"), 1.0);

        // Arrows + participant = 0.8
        assert!(detector.confidence("participant Alice\nAlice->>Bob: Hi") >= 0.8);

        // Just arrows = 0.6
        assert!(detector.confidence("Alice->>Bob: Hello") >= 0.6);

        // Nothing = 0.0
        assert_eq!(detector.confidence("graph TD; A-->B"), 0.0);
    }

    #[test]
    fn test_rejects_flowchart() {
        let detector = SequenceDetector::new();
        assert!(!detector.detect("graph TD; A-->B-->C"));
        assert!(!detector.detect("flowchart LR; A-->B"));
    }

    #[test]
    fn test_rejects_gitgraph() {
        let detector = SequenceDetector::new();
        assert!(!detector.detect("gitGraph\n    commit\n    commit"));
    }
}
