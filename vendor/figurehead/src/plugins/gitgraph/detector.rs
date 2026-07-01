//! Git graph detector implementation
//!
//! Detects git graph diagram syntax patterns.

use crate::core::Detector;
use tracing::{debug, info, trace};

/// Git graph detector implementation
pub struct GitGraphDetector;

impl Default for GitGraphDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl GitGraphDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Detector for GitGraphDetector {
    fn detect(&self, input: &str) -> bool {
        let input = input.trim();
        let input_len = input.len();

        trace!(input_len, "GitGraphDetector::detect called");

        if input.is_empty() {
            debug!("Empty input, detection failed");
            return false;
        }

        // Check for explicit git graph keywords (case-insensitive)
        let input_lower = input.to_lowercase();
        if input_lower.contains("gitgraph") || input_lower.contains("git graph") {
            info!("Detected git graph via keyword");
            return true;
        }

        // Check for Mermaid git graph commands
        if input_lower.contains("commit")
            && (input_lower.contains("branch")
                || input_lower.contains("merge")
                || input_lower.contains("checkout"))
        {
            debug!("Detected git graph via git commands");
            return true;
        }

        trace!("No git graph patterns detected");
        false
    }

    fn confidence(&self, input: &str) -> f64 {
        let input = input.trim();

        if input.is_empty() {
            return 0.0;
        }

        let mut score: f64 = 0.0;

        let input_lower = input.to_lowercase();

        // Primary indicators
        if input_lower.contains("gitgraph") || input_lower.contains("git graph") {
            score += 0.8;
        }

        // Secondary indicators - git commands
        if input_lower.contains("commit") {
            score += 0.3;
        }
        if input_lower.contains("branch") {
            score += 0.2;
        }
        if input_lower.contains("merge") {
            score += 0.2;
        }
        if input_lower.contains("checkout") || input_lower.contains("switch") {
            score += 0.1;
        }

        // Penalty for competing diagram types
        if input.contains("graph TD")
            || input.contains("graph LR")
            || input.contains("sequenceDiagram")
            || input.contains("classDiagram")
        {
            score *= 0.2; // Strong penalty for flowchart syntax
        }

        score.min(1.0)
    }

    fn diagram_type(&self) -> &'static str {
        "gitgraph"
    }

    fn patterns(&self) -> Vec<&'static str> {
        vec![
            "gitGraph",
            "git graph",
            "commit",
            "branch",
            "merge",
            "checkout",
            "switch",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_git_graph_keyword() {
        let detector = GitGraphDetector::new();
        assert!(detector.detect("gitGraph"));
        assert!(detector.detect("git graph"));
    }

    #[test]
    fn test_detects_git_commands() {
        let detector = GitGraphDetector::new();
        assert!(detector.detect("gitGraph\n   commit\n   commit"));
        assert!(detector.detect("commit\n   branch develop\n   merge main"));
    }

    #[test]
    fn test_confidence_scoring() {
        let detector = GitGraphDetector::new();
        assert!(detector.confidence("gitGraph") > 0.5);
        assert!(detector.confidence("commit\n   branch develop") > 0.3);
    }

    #[test]
    fn test_rejects_non_git_graph() {
        let detector = GitGraphDetector::new();
        assert!(!detector.detect("graph TD"));
        assert!(!detector.detect("A --> B"));
    }
}
