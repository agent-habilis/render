//! Class diagram detector
//!
//! Identifies class diagram syntax from input text.

use crate::core::Detector;

/// Detector for class diagram syntax
pub struct ClassDetector;

impl ClassDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClassDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ClassDetector {
    fn detect(&self, input: &str) -> bool {
        self.confidence(input) > 0.5
    }

    fn confidence(&self, input: &str) -> f64 {
        let input_lower = input.to_lowercase();
        let trimmed = input_lower.trim();

        // Must start with "classdiagram"
        if trimmed.starts_with("classdiagram") {
            return 1.0;
        }

        // Check for class-specific patterns
        let has_class_def = input.contains("class ") && input.contains('{');
        let has_inheritance = input.contains("<|--");
        let has_composition = input.contains("*--");
        let has_aggregation = input.contains("o--");
        let has_association = input.contains("-->");
        let has_dependency = input.contains("..>");
        let has_realization = input.contains("..|>");

        let has_relationship = has_inheritance
            || has_composition
            || has_aggregation
            || has_dependency
            || has_realization;

        // Class with braces = high confidence
        if has_class_def {
            return 0.8;
        }

        // Relationship patterns = moderate confidence
        if has_relationship {
            return 0.7;
        }

        // Generic association is too common (flowcharts use -->)
        // Only match if combined with other class indicators
        if has_association && input_lower.contains("class ") {
            return 0.6;
        }

        0.0
    }

    fn diagram_type(&self) -> &'static str {
        "class"
    }

    fn patterns(&self) -> Vec<&'static str> {
        vec!["classDiagram"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_class_diagram_keyword() {
        let detector = ClassDetector::new();
        assert!(detector.detect("classDiagram\n    class Animal"));
        assert!(detector.detect("classdiagram\n    class Animal"));
        assert!(detector.detect("CLASSDIAGRAM\n    class Animal"));
    }

    #[test]
    fn test_detects_class_patterns() {
        let detector = ClassDetector::new();
        // Class definitions without keyword
        assert!(detector.detect("class Animal {\n    +name: string\n}"));
        // Inheritance
        assert!(detector.detect("Animal <|-- Dog"));
        // Composition
        assert!(detector.detect("Person *-- Heart"));
    }

    #[test]
    fn test_confidence_scoring() {
        let detector = ClassDetector::new();

        // Full keyword = 1.0
        assert_eq!(detector.confidence("classDiagram\n    class Animal"), 1.0);

        // Class definition = 0.8
        assert!(detector.confidence("class Animal {\n    +name\n}") >= 0.7);

        // Relationship patterns = 0.7
        assert!(detector.confidence("Animal <|-- Dog") >= 0.7);

        // Nothing = 0.0
        assert_eq!(detector.confidence("graph TD; A-->B"), 0.0);
    }

    #[test]
    fn test_rejects_flowchart() {
        let detector = ClassDetector::new();
        assert!(!detector.detect("graph TD; A-->B-->C"));
        assert!(!detector.detect("flowchart LR; A-->B"));
    }

    #[test]
    fn test_rejects_sequence() {
        let detector = ClassDetector::new();
        assert!(!detector.detect("sequenceDiagram\n    Alice->>Bob: Hello"));
    }

    #[test]
    fn test_rejects_gitgraph() {
        let detector = ClassDetector::new();
        assert!(!detector.detect("gitGraph\n    commit\n    commit"));
    }
}
