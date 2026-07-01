//! Git graph syntax parser
//!
//! Parses Mermaid.js git graph syntax into syntax nodes.
//!
//! Supported syntax:
//! - `gitGraph` keyword to start
//! - `commit` to add commits (with optional `id: "..."`, `type: NORMAL|REVERSE|HIGHLIGHT`, `tag: "..."`)
//! - `branch <name>` to create and checkout a new branch
//! - `checkout <name>` to switch to an existing branch
//! - `merge <name>` to merge a branch into current branch

use crate::core::{SyntaxMetadata, SyntaxNode, SyntaxParser};
use anyhow::Result;
use tracing::{debug, trace};

/// Git graph syntax parser
pub struct GitGraphSyntaxParser;

impl GitGraphSyntaxParser {
    pub fn new() -> Self {
        Self
    }

    fn parse_commit_attributes(line: &str) -> (Option<String>, Option<String>, Option<String>) {
        // Parse: commit id: "Alpha" type: HIGHLIGHT tag: "v1.0"
        let mut id = None;
        let mut commit_type = None;
        let mut tag = None;

        // Extract id: "value"
        if let Some(id_start) = line.find("id:") {
            if let Some(quote_start) = line[id_start..].find('"') {
                let start = id_start + quote_start + 1;
                if let Some(quote_end) = line[start..].find('"') {
                    id = Some(line[start..start + quote_end].to_string());
                }
            }
        }

        // Extract type: VALUE
        if let Some(type_start) = line.find("type:") {
            let after_colon = &line[type_start + 5..].trim();
            let type_value = after_colon
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_uppercase();
            if !type_value.is_empty() {
                commit_type = Some(type_value);
            }
        }

        // Extract tag: "value"
        if let Some(tag_start) = line.find("tag:") {
            if let Some(quote_start) = line[tag_start..].find('"') {
                let start = tag_start + quote_start + 1;
                if let Some(quote_end) = line[start..].find('"') {
                    tag = Some(line[start..start + quote_end].to_string());
                }
            }
        }

        (id, commit_type, tag)
    }
}

impl SyntaxParser for GitGraphSyntaxParser {
    fn parse(&self, input: &str) -> Result<Vec<SyntaxNode>> {
        trace!("Parsing git graph syntax");
        let mut nodes = Vec::new();
        let mut current_branch = "main".to_string();
        let mut branches: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        branches.insert("main".to_string(), Vec::new());
        let mut commit_counter = 0;

        // Split input into lines
        let lines: Vec<&str> = input
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            return Ok(nodes);
        }

        // Skip gitGraph keyword if present
        let mut line_iter = lines.iter().peekable();
        if let Some(first_line) = line_iter.peek() {
            if first_line.trim().eq_ignore_ascii_case("gitgraph") {
                line_iter.next();
            }
        }

        // Parse each command
        for line in line_iter {
            let line_lower = line.to_lowercase();

            if line_lower.starts_with("commit") {
                // Parse commit command
                let (id, commit_type, tag) = Self::parse_commit_attributes(line);
                let commit_id = id.clone().unwrap_or_else(|| {
                    commit_counter += 1;
                    format!("c{}", commit_counter)
                });

                let commit_type_str = commit_type.unwrap_or_else(|| "NORMAL".to_string());

                let mut metadata = SyntaxMetadata::new()
                    .with_attr("type", "commit")
                    .with_attr("commit_type", &commit_type_str);

                if let Some(tag_val) = &tag {
                    metadata = metadata.with_attr("tag", tag_val);
                }

                let commit_label = id;
                nodes.push(SyntaxNode::Node {
                    id: commit_id.clone(),
                    label: commit_label,
                    metadata,
                });

                // Add commit to current branch and create edge
                let prev_commit_opt = {
                    let branch_commits = branches.get_mut(&current_branch).unwrap();
                    let prev = if branch_commits.is_empty() {
                        None
                    } else {
                        Some(branch_commits.last().unwrap().clone())
                    };
                    branch_commits.push(commit_id.clone());
                    prev
                };

                // Create edge from previous commit on this branch if exists
                if let Some(prev_commit) = prev_commit_opt {
                    nodes.push(SyntaxNode::Edge {
                        from: prev_commit,
                        to: commit_id,
                        label: None,
                        metadata: SyntaxMetadata::new().with_attr("type", "parent"),
                    });
                }
            } else if line_lower.starts_with("branch") {
                // Parse branch command: branch develop
                let branch_name = line[6..].trim().trim_matches('"').to_string();
                if !branches.contains_key(&branch_name) {
                    branches.insert(branch_name.clone(), Vec::new());
                    nodes.push(SyntaxNode::Node {
                        id: format!("branch_{}", branch_name),
                        label: Some(branch_name.clone()),
                        metadata: SyntaxMetadata::new().with_attr("type", "branch"),
                    });
                }
                current_branch = branch_name;
            } else if line_lower.starts_with("checkout") || line_lower.starts_with("switch") {
                // Parse checkout command: checkout develop
                let branch_name = if line_lower.starts_with("checkout") {
                    line[8..].trim().trim_matches('"').to_string()
                } else {
                    line[6..].trim().trim_matches('"').to_string()
                };
                if branches.contains_key(&branch_name) {
                    current_branch = branch_name;
                }
            } else if line_lower.starts_with("merge") {
                // Parse merge command: merge develop
                let branch_name = line[5..].trim().trim_matches('"').to_string();
                let (last_merged_opt, prev_commit_opt) = {
                    let merged_commits = branches.get(&branch_name);
                    let last_merged = merged_commits.and_then(|c| c.last()).cloned();

                    let prev_commit = branches
                        .get(&current_branch)
                        .and_then(|c| if !c.is_empty() { c.last() } else { None })
                        .cloned();

                    (last_merged, prev_commit)
                };

                if let Some(last_merged) = last_merged_opt {
                    // Create merge commit
                    commit_counter += 1;
                    let merge_commit_id = format!("c{}", commit_counter);
                    nodes.push(SyntaxNode::Node {
                        id: merge_commit_id.clone(),
                        label: None,
                        metadata: SyntaxMetadata::new()
                            .with_attr("type", "commit")
                            .with_attr("commit_type", "MERGE"),
                    });

                    branches
                        .get_mut(&current_branch)
                        .unwrap()
                        .push(merge_commit_id.clone());

                    // Edge from merged branch's last commit
                    nodes.push(SyntaxNode::Edge {
                        from: last_merged,
                        to: merge_commit_id.clone(),
                        label: None,
                        metadata: SyntaxMetadata::new().with_attr("type", "merge"),
                    });

                    // Edge from current branch's previous commit if exists
                    if let Some(prev_commit) = prev_commit_opt {
                        nodes.push(SyntaxNode::Edge {
                            from: prev_commit,
                            to: merge_commit_id,
                            label: None,
                            metadata: SyntaxMetadata::new().with_attr("type", "parent"),
                        });
                    }
                }
            }
        }

        debug!(
            commit_count = commit_counter,
            branch_count = branches.len(),
            "Parsed git graph"
        );
        Ok(nodes)
    }

    fn name(&self) -> &'static str {
        "gitgraph"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        let input_lower = input.to_lowercase();
        input_lower.contains("gitgraph")
            || (input_lower.contains("commit")
                && (input_lower.contains("branch") || input_lower.contains("merge")))
    }
}

impl Default for GitGraphSyntaxParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_commits() {
        let parser = GitGraphSyntaxParser::new();
        let input = "gitGraph\n   commit\n   commit\n   commit";
        let nodes = parser.parse(input).unwrap();

        // Should have 3 commits and 2 edges
        let commit_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, SyntaxNode::Node { .. }))
            .collect();
        let edges: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, SyntaxNode::Edge { .. }))
            .collect();

        assert_eq!(commit_nodes.len(), 3);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_parse_with_branches() {
        let parser = GitGraphSyntaxParser::new();
        let input = r#"gitGraph
   commit
   commit
   branch develop
   checkout develop
   commit
   commit"#;
        let nodes = parser.parse(input).unwrap();

        let commit_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, SyntaxNode::Node { .. }))
            .collect();

        assert!(commit_nodes.len() >= 4); // At least 4 commits
    }

    #[test]
    fn test_parse_with_merge() {
        let parser = GitGraphSyntaxParser::new();
        let input = r#"gitGraph
   commit
   branch develop
   checkout develop
   commit
   checkout main
   merge develop"#;
        let nodes = parser.parse(input).unwrap();

        let commit_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, SyntaxNode::Node { .. }))
            .collect();

        assert!(commit_nodes.len() >= 2);
    }

    #[test]
    fn test_can_parse() {
        let parser = GitGraphSyntaxParser::new();
        assert!(parser.can_parse("gitGraph\n   commit"));
        assert!(parser.can_parse("commit\n   branch develop"));
        assert!(!parser.can_parse("A --> B"));
    }
}
