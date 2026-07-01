//! Flowchart layout implementation
//!
//! Arranges flowchart elements in a coordinate system using a Sugiyama-style
//! layered layout algorithm.

use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info, span, trace, Level};
use unicode_width::UnicodeWidthStr;

use super::FlowchartDatabase;
use crate::core::{wrap_label, Database, Direction, LayoutAlgorithm, NodeShape};

/// Position data for a laid out node
#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub id: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Position data for a laid out edge
#[derive(Debug, Clone)]
pub struct PositionedEdge {
    pub from_id: String,
    pub to_id: String,
    pub waypoints: Vec<(usize, usize)>,
    /// For grouped edges from same source (split), the shared junction point
    pub junction: Option<(usize, usize)>,
    /// For grouped edges to same target (merge), the shared junction point
    pub merge_junction: Option<(usize, usize)>,
    /// Index within the edge group (0 = first/leftmost in TD)
    pub group_index: Option<usize>,
    /// Total edges in this group
    pub group_size: Option<usize>,
}

/// Position data for a laid out subgraph (container)
#[derive(Debug, Clone)]
pub struct PositionedSubgraph {
    pub id: String,
    pub title: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Layout output containing positioned elements
#[derive(Debug)]
pub struct FlowchartLayoutResult {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
    pub subgraphs: Vec<PositionedSubgraph>,
    pub width: usize,
    pub height: usize,
}

/// Layout configuration
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub node_sep: usize,
    pub rank_sep: usize,
    pub min_node_width: usize,
    pub min_node_height: usize,
    pub padding: usize,
    pub max_label_width: usize, // Max width before label wraps (0 = no wrap)
    pub diamond_style: crate::core::DiamondStyle,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            node_sep: 1, // was 4: horizontal gap between nodes in same layer
            rank_sep: 4, // gap between layers (need 4 for visible edge lines in LR splits)
            min_node_width: 5,
            min_node_height: 3,
            padding: 1,          // was 2: canvas edge padding
            max_label_width: 30, // Wrap labels longer than 30 chars
            diamond_style: crate::core::DiamondStyle::Box,
        }
    }
}

/// Flowchart layout algorithm implementation
pub struct FlowchartLayoutAlgorithm {
    config: LayoutConfig,
}

impl FlowchartLayoutAlgorithm {
    pub fn new() -> Self {
        Self {
            config: LayoutConfig::default(),
        }
    }

    pub fn with_config(config: LayoutConfig) -> Self {
        Self { config }
    }

    /// Get mutable access to the layout configuration
    pub fn config_mut(&mut self) -> &mut LayoutConfig {
        &mut self.config
    }

    /// Wrap a label into multiple lines if it exceeds max_label_width
    fn wrap_label(&self, label: &str) -> Vec<String> {
        wrap_label(label, self.config.max_label_width)
    }

    /// Calculate node dimensions based on shape and label
    fn calculate_node_size(&self, label: &str, shape: NodeShape) -> (usize, usize) {
        let wrapped_lines = self.wrap_label(label);
        let label_width = wrapped_lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()))
            .max()
            .unwrap_or(0);
        let label_lines = wrapped_lines.len();

        let (extra_width, extra_height): (usize, i32) = match shape {
            NodeShape::Rectangle | NodeShape::RoundedRect | NodeShape::Subroutine => (4, 0),
            NodeShape::Diamond => {
                // Diamond height depends on the diamond style
                use crate::core::DiamondStyle;
                let height_extra = match self.config.diamond_style {
                    DiamondStyle::Box => 0,     // 3 lines total
                    DiamondStyle::Inline => -2, // 1 line total (will be clamped to min)
                    DiamondStyle::Tall => 2,    // 5 lines total
                };
                (6, height_extra)
            }
            NodeShape::Circle | NodeShape::Terminal => (4, 0),
            NodeShape::Hexagon => (6, 0),
            NodeShape::Asymmetric | NodeShape::Parallelogram | NodeShape::Trapezoid => (6, 0),
            NodeShape::Cylinder => (6, 2),
        };

        let width = (label_width + extra_width).max(self.config.min_node_width);
        // Add extra height for multi-line labels (each extra line adds 1)
        let base_height = (3i32 + extra_height).max(1) as usize;
        let height = (base_height + label_lines.saturating_sub(1)).max(self.config.min_node_height);

        (width, height)
    }
}

impl Default for FlowchartLayoutAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutAlgorithm<FlowchartDatabase> for FlowchartLayoutAlgorithm {
    type Output = FlowchartLayoutResult;

    fn layout(&self, database: &FlowchartDatabase) -> Result<Self::Output> {
        let layout_span = span!(
            Level::INFO,
            "layout_flowchart",
            node_count = database.node_count(),
            edge_count = database.edge_count(),
            direction = ?database.direction()
        );
        let _enter = layout_span.enter(); // Enter span to track duration

        trace!("Starting flowchart layout");

        let direction = database.direction();

        // Collect nodes and calculate sizes
        let size_span = span!(Level::DEBUG, "calculate_node_sizes");
        let _size_enter = size_span.enter();
        let nodes: Vec<_> = database.nodes().collect();
        if nodes.is_empty() {
            debug!("Empty database, returning empty layout");
            return Ok(FlowchartLayoutResult {
                nodes: Vec::new(),
                edges: Vec::new(),
                subgraphs: Vec::new(),
                width: 0,
                height: 0,
            });
        }

        let mut node_sizes: HashMap<&str, (usize, usize)> = HashMap::new();
        for node in &nodes {
            let size = self.calculate_node_size(&node.label, node.shape);
            node_sizes.insert(&node.id, size);
        }
        debug!(node_count = nodes.len(), "Calculated node sizes");
        drop(_size_enter);

        // Assign layers using topological sort
        let layer_span = span!(Level::DEBUG, "assign_layers");
        let _layer_enter = layer_span.enter();
        let sorted = database.topological_sort();
        let mut layers: HashMap<&str, usize> = HashMap::new();

        for &node_id in &sorted {
            // Layer = max layer of predecessors + 1
            let preds = database.predecessors(node_id);
            let layer = if preds.is_empty() {
                0
            } else {
                preds
                    .iter()
                    .filter_map(|&p| layers.get(p))
                    .max()
                    .map(|&l| l + 1)
                    .unwrap_or(0)
            };
            layers.insert(node_id, layer);
        }

        // Group nodes by layer
        let max_layer = layers.values().max().copied().unwrap_or(0);
        let mut layer_nodes: Vec<Vec<&str>> = vec![Vec::new(); max_layer + 1];
        for (&node_id, &layer) in &layers {
            layer_nodes[layer].push(node_id);
        }

        // Initial sort for determinism, then apply barycenter ordering
        for layer in &mut layer_nodes {
            layer.sort();
        }

        // Apply barycenter ordering to minimize edge crossings
        let crossing_count =
            super::ordering::order_layers_barycenter(database, &mut layer_nodes, 4);

        debug!(
            max_layer,
            layer_count = layer_nodes.len(),
            crossing_count,
            "Assigned nodes to layers with barycenter ordering"
        );
        drop(_layer_enter);

        // Normalize node widths within layers for LR/RL direction (for alignment)
        // TD/BU keeps natural heights - shapes extend as needed
        match direction {
            Direction::TopDown | Direction::BottomUp => {
                // Keep natural heights - no normalization needed
            }
            Direction::LeftRight | Direction::RightLeft => {
                // Normalize widths within layers
                let mut layer_max_widths: HashMap<usize, usize> = HashMap::new();
                for &node_id in &sorted {
                    if let (Some(&layer), Some(&(width, _))) =
                        (layers.get(node_id), node_sizes.get(node_id))
                    {
                        let max = layer_max_widths.entry(layer).or_insert(0);
                        *max = (*max).max(width);
                    }
                }
                for &node_id in &sorted {
                    if let (Some(&layer), Some((_, height))) =
                        (layers.get(node_id), node_sizes.get(node_id).copied())
                    {
                        if let Some(&max_width) = layer_max_widths.get(&layer) {
                            node_sizes.insert(node_id, (max_width, height));
                        }
                    }
                }
            }
        }

        // Calculate positions based on direction
        let position_span = span!(Level::DEBUG, "calculate_positions", direction = ?direction);
        let _position_enter = position_span.enter();
        let mut positioned_nodes = Vec::new();
        let mut max_width = 0;
        let mut max_height = 0;

        match direction {
            Direction::TopDown | Direction::BottomUp => {
                // Vertical layout: layers are rows (Y), nodes distributed on X
                // Find the widest layer (sum of node widths + gaps) for centering
                let widest_layer_width = layer_nodes
                    .iter()
                    .map(|layer| {
                        let total: usize = layer.iter().map(|&id| node_sizes[id].0).sum();
                        total + layer.len().saturating_sub(1) * self.config.node_sep
                    })
                    .max()
                    .unwrap_or(0);
                // Center X is at padding + widest_layer_width / 2
                let center_x = self.config.padding + widest_layer_width / 2;

                let mut y = self.config.padding;

                let layer_iter: Box<dyn Iterator<Item = &Vec<&str>>> = if direction.is_reversed() {
                    Box::new(layer_nodes.iter().rev())
                } else {
                    Box::new(layer_nodes.iter())
                };

                for layer in layer_iter {
                    let mut layer_height = 0;

                    if layer.len() == 1 {
                        // Single node - center it on the center line
                        let node_id = layer[0];
                        let (width, height) = node_sizes[node_id];
                        let x = center_x.saturating_sub(width / 2);
                        positioned_nodes.push(PositionedNode {
                            id: node_id.to_string(),
                            x,
                            y,
                            width,
                            height,
                        });
                        layer_height = height;
                        max_width = max_width.max(x + width + self.config.padding);
                    } else {
                        // Multiple nodes - distribute across from center
                        let total_width: usize =
                            layer.iter().map(|&id| node_sizes[id].0).sum::<usize>()
                                + (layer.len() - 1) * self.config.node_sep;
                        let start_x = center_x.saturating_sub(total_width / 2);
                        let mut x = start_x;

                        for &node_id in layer {
                            let (width, height) = node_sizes[node_id];
                            positioned_nodes.push(PositionedNode {
                                id: node_id.to_string(),
                                x,
                                y,
                                width,
                                height,
                            });

                            x += width + self.config.node_sep;
                            layer_height = layer_height.max(height);
                            max_width = max_width.max(x);
                        }
                    }

                    y += layer_height + self.config.rank_sep;
                    max_height = max_height.max(y);
                }
            }
            Direction::LeftRight | Direction::RightLeft => {
                // Horizontal layout: layers are columns (X), nodes distributed on Y
                // First, calculate the maximum height needed for any layer
                let mut layer_max_heights: Vec<usize> = Vec::new();
                for layer in &layer_nodes {
                    let layer_height: usize =
                        layer.iter().map(|&id| node_sizes[id].1).sum::<usize>()
                            + layer.len().saturating_sub(1) * self.config.node_sep;
                    layer_max_heights.push(layer_height);
                }
                let total_max_height = *layer_max_heights.iter().max().unwrap_or(&0);

                let mut x = self.config.padding;

                let layer_iter: Box<dyn Iterator<Item = (usize, &Vec<&str>)>> =
                    if direction.is_reversed() {
                        Box::new(layer_nodes.iter().enumerate().rev())
                    } else {
                        Box::new(layer_nodes.iter().enumerate())
                    };

                for (layer_idx, layer) in layer_iter {
                    // Calculate total height of this layer's nodes
                    let layer_height = layer_max_heights[layer_idx];
                    // Center the layer vertically
                    let start_y =
                        self.config.padding + (total_max_height.saturating_sub(layer_height)) / 2;
                    let mut y = start_y;
                    let mut layer_width = 0;

                    for &node_id in layer {
                        let (width, height) = node_sizes[node_id];
                        positioned_nodes.push(PositionedNode {
                            id: node_id.to_string(),
                            x,
                            y,
                            width,
                            height,
                        });

                        y += height + self.config.node_sep;
                        layer_width = layer_width.max(width);
                        max_height = max_height.max(y);
                    }

                    x += layer_width + self.config.rank_sep;
                    max_width = max_width.max(x);
                }
                // Ensure max_height accounts for the centered layout
                max_height = max_height.max(self.config.padding + total_max_height);
            }
        }

        debug!(
            positioned_node_count = positioned_nodes.len(),
            max_width, max_height, "Node positioning completed"
        );
        drop(_position_enter);

        // Route edges with grouping for splits and merges
        let edge_span = span!(Level::DEBUG, "route_edges");
        let _edge_enter = edge_span.enter();

        // Group edges by source node (for splits)
        let mut edges_by_source: HashMap<&str, Vec<&crate::core::EdgeData>> = HashMap::new();
        // Group edges by target node (for merges)
        let mut edges_by_target: HashMap<&str, Vec<&crate::core::EdgeData>> = HashMap::new();
        for edge in database.edges() {
            edges_by_source.entry(&edge.from).or_default().push(edge);
            edges_by_target.entry(&edge.to).or_default().push(edge);
        }

        let mut positioned_edges = Vec::new();
        let node_positions: HashMap<&str, &PositionedNode> = positioned_nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        // Pre-calculate merge junctions for targets with multiple incoming FORWARD edges
        // (back-edges enter from the side and shouldn't count toward merge junctions)
        let mut merge_junctions: HashMap<&str, (usize, usize)> = HashMap::new();
        for (target_id, incoming_edges) in &edges_by_target {
            if let Some(to) = node_positions.get(*target_id) {
                // Count only forward edges (not back-edges)
                let forward_edge_count = incoming_edges
                    .iter()
                    .filter(|edge| {
                        if let Some(from) = node_positions.get(edge.from.as_str()) {
                            // Forward edge: source is "before" target in flow direction
                            match direction {
                                Direction::TopDown => from.y < to.y,
                                Direction::BottomUp => from.y > to.y,
                                Direction::LeftRight => from.x < to.x,
                                Direction::RightLeft => from.x > to.x,
                            }
                        } else {
                            false
                        }
                    })
                    .count();

                if forward_edge_count > 1 {
                    let merge_point = match direction {
                        Direction::TopDown => (to.x + to.width / 2, to.y.saturating_sub(2)),
                        Direction::BottomUp => (to.x + to.width / 2, to.y + to.height + 2),
                        Direction::LeftRight => (to.x.saturating_sub(2), to.y + to.height / 2),
                        Direction::RightLeft => (to.x + to.width + 2, to.y + to.height / 2),
                    };
                    merge_junctions.insert(*target_id, merge_point);
                }
            }
        }

        for (source_id, edges) in edges_by_source {
            let Some(from) = node_positions.get(source_id) else {
                continue;
            };

            let group_size = edges.len();
            let is_split = group_size > 1;

            // Calculate junction point for splits
            let junction = if is_split {
                match direction {
                    Direction::TopDown => Some((from.x + from.width / 2, from.y + from.height + 1)),
                    Direction::BottomUp => {
                        Some((from.x + from.width / 2, from.y.saturating_sub(1)))
                    }
                    Direction::LeftRight => {
                        Some((from.x + from.width + 1, from.y + from.height / 2))
                    }
                    Direction::RightLeft => {
                        Some((from.x.saturating_sub(1), from.y + from.height / 2))
                    }
                }
            } else {
                None
            };

            // Sort edges for consistent ordering (by target position)
            let mut sorted_edges: Vec<_> = edges.into_iter().collect();
            sorted_edges.sort_by_key(|e| {
                node_positions
                    .get(e.to.as_str())
                    .map(|n| (n.x, n.y))
                    .unwrap_or((usize::MAX, usize::MAX))
            });

            for (group_index, edge) in sorted_edges.into_iter().enumerate() {
                let Some(to) = node_positions.get(edge.to.as_str()) else {
                    continue;
                };

                // Check if this edge is part of a merge
                let merge_junction = merge_junctions.get(edge.to.as_str()).copied();

                // Detect back-edges (edges going against the flow direction)
                // Back-edge: when normal routing would require going "backwards"
                // In TopDown: target top <= source bottom means edge goes UP
                let is_back_edge = match direction {
                    Direction::TopDown => to.y < from.y,   // target above source
                    Direction::BottomUp => to.y > from.y,  // target below source
                    Direction::LeftRight => to.x < from.x, // target left of source
                    Direction::RightLeft => to.x > from.x, // target right of source
                };

                // Calculate exit and entry points (and waypoints for back-edges)
                let waypoints = if is_back_edge {
                    // Route back-edges around the diagram
                    let route_x = max_width + 2; // Route 2 cells past right edge
                    match direction {
                        Direction::TopDown => {
                            // Exit from right side, go right, up, left, enter from right
                            let exit_x = from.x + from.width;
                            let exit_y = from.y + from.height / 2;
                            let entry_x = to.x + to.width;
                            let entry_y = to.y + to.height / 2;
                            vec![
                                (exit_x, exit_y),
                                (route_x, exit_y),
                                (route_x, entry_y),
                                (entry_x, entry_y),
                            ]
                        }
                        Direction::BottomUp => {
                            let exit_x = from.x + from.width;
                            let exit_y = from.y + from.height / 2;
                            let entry_x = to.x + to.width;
                            let entry_y = to.y + to.height / 2;
                            vec![
                                (exit_x, exit_y),
                                (route_x, exit_y),
                                (route_x, entry_y),
                                (entry_x, entry_y),
                            ]
                        }
                        Direction::LeftRight => {
                            let route_y = max_height + 2;
                            let exit_x = from.x + from.width / 2;
                            let exit_y = from.y + from.height;
                            let entry_x = to.x + to.width / 2;
                            let entry_y = to.y + to.height;
                            vec![
                                (exit_x, exit_y),
                                (exit_x, route_y),
                                (entry_x, route_y),
                                (entry_x, entry_y),
                            ]
                        }
                        Direction::RightLeft => {
                            let route_y = max_height + 2;
                            let exit_x = from.x + from.width / 2;
                            let exit_y = from.y + from.height;
                            let entry_x = to.x + to.width / 2;
                            let entry_y = to.y + to.height;
                            vec![
                                (exit_x, exit_y),
                                (exit_x, route_y),
                                (entry_x, route_y),
                                (entry_x, entry_y),
                            ]
                        }
                    }
                } else {
                    // Normal forward edge
                    let (exit_x, exit_y, entry_x, entry_y) = match direction {
                        Direction::TopDown => (
                            from.x + from.width / 2,
                            from.y + from.height,
                            to.x + to.width / 2,
                            to.y,
                        ),
                        Direction::BottomUp => (
                            from.x + from.width / 2,
                            from.y,
                            to.x + to.width / 2,
                            to.y + to.height,
                        ),
                        Direction::LeftRight => (
                            from.x + from.width,
                            from.y + from.height / 2,
                            to.x,
                            to.y + to.height / 2,
                        ),
                        Direction::RightLeft => (
                            from.x,
                            from.y + from.height / 2,
                            to.x + to.width,
                            to.y + to.height / 2,
                        ),
                    };
                    vec![(exit_x, exit_y), (entry_x, entry_y)]
                };

                positioned_edges.push(PositionedEdge {
                    from_id: edge.from.clone(),
                    to_id: edge.to.clone(),
                    waypoints,
                    junction,
                    merge_junction,
                    group_index: if is_split { Some(group_index) } else { None },
                    group_size: if is_split { Some(group_size) } else { None },
                });
            }
        }
        debug!(
            positioned_edge_count = positioned_edges.len(),
            "Edge routing completed"
        );
        drop(_edge_enter);

        // Calculate subgraph bounding boxes from member node positions
        let subgraph_span = span!(Level::DEBUG, "calculate_subgraphs");
        let _subgraph_enter = subgraph_span.enter();

        // Build a lookup for positioned nodes by ID
        let node_positions: HashMap<&str, &PositionedNode> = positioned_nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        let mut positioned_subgraphs = Vec::new();
        for subgraph in database.subgraphs() {
            if subgraph.members.is_empty() {
                // Empty subgraph: render as minimal box at top-left with padding
                positioned_subgraphs.push(PositionedSubgraph {
                    id: subgraph.id.clone(),
                    title: subgraph.title.clone(),
                    x: self.config.padding,
                    y: self.config.padding,
                    // Width: title + padding for borders
                    width: unicode_width::UnicodeWidthStr::width(subgraph.title.as_str()) + 4,
                    height: 3, // Just title bar + empty interior
                });
                continue;
            }

            // Find bounding box of all member nodes
            let mut min_x = usize::MAX;
            let mut min_y = usize::MAX;
            let mut max_x = 0usize;
            let mut max_y = 0usize;

            for member_id in &subgraph.members {
                if let Some(node) = node_positions.get(member_id.as_str()) {
                    min_x = min_x.min(node.x);
                    min_y = min_y.min(node.y);
                    max_x = max_x.max(node.x + node.width);
                    max_y = max_y.max(node.y + node.height);
                }
            }

            if min_x == usize::MAX {
                // No members found (shouldn't happen, but defensive)
                continue;
            }

            // Add padding for border: more breathing room around nodes
            let border_padding = 2; // padding around nodes inside subgraph
            let title_height = 1; // row for title

            positioned_subgraphs.push(PositionedSubgraph {
                id: subgraph.id.clone(),
                title: subgraph.title.clone(),
                x: min_x.saturating_sub(border_padding),
                y: min_y.saturating_sub(border_padding + title_height),
                width: (max_x - min_x) + border_padding * 2,
                height: (max_y - min_y) + border_padding * 2 + title_height,
            });
        }
        debug!(
            subgraph_count = positioned_subgraphs.len(),
            "Subgraph bounding boxes calculated"
        );
        drop(_subgraph_enter);

        // Check if any back-edges need extra width for routing around diagram
        let has_back_edges = positioned_edges.iter().any(|e| e.waypoints.len() > 2);
        let back_edge_margin = if has_back_edges { 4 } else { 0 }; // route_x uses max_width + 2

        let final_width = max_width + self.config.padding + back_edge_margin;
        let final_height = max_height + self.config.padding;
        info!(
            node_count = positioned_nodes.len(),
            edge_count = positioned_edges.len(),
            subgraph_count = positioned_subgraphs.len(),
            width = final_width,
            height = final_height,
            "Layout completed"
        );

        Ok(FlowchartLayoutResult {
            nodes: positioned_nodes,
            edges: positioned_edges,
            subgraphs: positioned_subgraphs,
            width: final_width,
            height: final_height,
        })
    }

    fn name(&self) -> &'static str {
        "sugiyama"
    }

    fn version(&self) -> &'static str {
        "0.2.0"
    }

    fn direction(&self) -> &'static str {
        "configurable"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_linear_layout_lr() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Process").unwrap();
        db.add_simple_node("C", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 3);
        assert!(result.width > 0);
        assert!(result.height > 0);

        let node_by_id: HashMap<_, _> = result.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // LR layout: x should increase left to right
        assert!(node_by_id["A"].x < node_by_id["B"].x);
        assert!(node_by_id["B"].x < node_by_id["C"].x);
    }

    #[test]
    fn test_basic_linear_layout_td() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Process").unwrap();
        db.add_simple_node("C", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        let node_by_id: HashMap<_, _> = result.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // TD layout: y should increase top to bottom
        assert!(node_by_id["A"].y < node_by_id["B"].y);
        assert!(node_by_id["B"].y < node_by_id["C"].y);
    }

    #[test]
    fn test_diamond_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Path1").unwrap();
        db.add_simple_node("C", "Path2").unwrap();
        db.add_simple_node("D", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();
        db.add_simple_edge("C", "D").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 4);

        let node_by_id: HashMap<_, _> = result.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // B and C should be in the same column (same x)
        assert_eq!(node_by_id["B"].x, node_by_id["C"].x);
        // B and C should have different y positions
        assert_ne!(node_by_id["B"].y, node_by_id["C"].y);
    }

    #[test]
    fn test_empty_database() {
        let db = FlowchartDatabase::new();
        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 0);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 0);
    }

    #[test]
    fn test_edges_are_routed() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].from_id, "A");
        assert_eq!(result.edges[0].to_id, "B");
        assert!(result.edges[0].waypoints.len() >= 2);
    }

    #[test]
    fn test_bottom_up_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::BottomUp);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Process").unwrap();
        db.add_simple_node("C", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        let node_by_id: HashMap<_, _> = result.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // BT layout: y should decrease bottom to top (higher y = lower in diagram)
        assert!(node_by_id["A"].y > node_by_id["B"].y);
        assert!(node_by_id["B"].y > node_by_id["C"].y);
    }

    #[test]
    fn test_right_left_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::RightLeft);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Process").unwrap();
        db.add_simple_node("C", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        let node_by_id: HashMap<_, _> = result.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // RL layout: x should decrease right to left
        assert!(node_by_id["A"].x > node_by_id["B"].x);
        assert!(node_by_id["B"].x > node_by_id["C"].x);
    }

    #[test]
    fn test_single_node_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_simple_node("A", "Single").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id, "A");
        assert!(result.width > 0);
        assert!(result.height > 0);
    }

    #[test]
    fn test_disconnected_nodes() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_simple_node("A", "Node A").unwrap();
        db.add_simple_node("B", "Node B").unwrap();
        db.add_simple_node("C", "Node C").unwrap();
        // No edges - all disconnected

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 0);
        // All nodes should be in layer 0 (no predecessors)
        // They should be positioned horizontally
    }

    #[test]
    fn test_self_loop() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_simple_node("A", "Loop").unwrap();
        db.add_simple_edge("A", "A").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].from_id, "A");
        assert_eq!(result.edges[0].to_id, "A");
    }

    #[test]
    fn test_multiple_edges_between_same_nodes() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_labeled_edge("A", "B", crate::core::EdgeType::DottedArrow, "Alternative")
            .unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 2);
        // Both edges should be present
        assert_eq!(result.edges.len(), 2);
    }

    #[test]
    fn test_complex_branching_pattern() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        // A -> B, C, D (three branches)
        // B -> E
        // C -> E
        // D -> E
        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Branch 1").unwrap();
        db.add_simple_node("C", "Branch 2").unwrap();
        db.add_simple_node("D", "Branch 3").unwrap();
        db.add_simple_node("E", "End").unwrap();

        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("B", "E").unwrap();
        db.add_simple_edge("C", "E").unwrap();
        db.add_simple_edge("D", "E").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 5);
        assert_eq!(result.edges.len(), 6);

        // B, C, D should be in the same layer (layer 1)
        let layer_b = result
            .nodes
            .iter()
            .position(|n| n.id == "B")
            .map(|i| result.nodes[i].y)
            .unwrap();
        let layer_c = result
            .nodes
            .iter()
            .position(|n| n.id == "C")
            .map(|i| result.nodes[i].y)
            .unwrap();
        let layer_d = result
            .nodes
            .iter()
            .position(|n| n.id == "D")
            .map(|i| result.nodes[i].y)
            .unwrap();
        assert_eq!(layer_b, layer_c);
        assert_eq!(layer_c, layer_d);
    }

    #[test]
    fn test_circular_dependency_handling() {
        let mut db = FlowchartDatabase::with_direction(Direction::LeftRight);

        // A -> B -> C -> A (cycle)
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();

        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("B", "C").unwrap();
        db.add_simple_edge("C", "A").unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        // Should handle cycles gracefully (topological sort may break ties arbitrarily)
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 3);
        // All nodes should be positioned
        assert!(result.width > 0);
        assert!(result.height > 0);
    }

    #[test]
    fn test_node_shapes_affect_sizing() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_shaped_node("A", "Short", crate::core::NodeShape::Rectangle)
            .unwrap();
        db.add_shaped_node(
            "B",
            "This is a very long label",
            crate::core::NodeShape::Rectangle,
        )
        .unwrap();
        db.add_shaped_node("C", "Diamond", crate::core::NodeShape::Diamond)
            .unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        let node_by_id: HashMap<_, _> = result.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // Long label should result in wider node
        assert!(node_by_id["B"].width >= node_by_id["A"].width);

        // Diamond should have extra height
        assert!(node_by_id["C"].height >= node_by_id["A"].height);
    }

    #[test]
    fn test_all_node_shapes_in_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_shaped_node("R", "Rect", crate::core::NodeShape::Rectangle)
            .unwrap();
        db.add_shaped_node("RR", "Rounded", crate::core::NodeShape::RoundedRect)
            .unwrap();
        db.add_shaped_node("D", "Diamond", crate::core::NodeShape::Diamond)
            .unwrap();
        db.add_shaped_node("C", "Circle", crate::core::NodeShape::Circle)
            .unwrap();
        db.add_shaped_node("S", "Subroutine", crate::core::NodeShape::Subroutine)
            .unwrap();
        db.add_shaped_node("H", "Hexagon", crate::core::NodeShape::Hexagon)
            .unwrap();
        db.add_shaped_node("Cy", "Cylinder", crate::core::NodeShape::Cylinder)
            .unwrap();
        db.add_shaped_node("P", "Parallelogram", crate::core::NodeShape::Parallelogram)
            .unwrap();
        db.add_shaped_node("T", "Trapezoid", crate::core::NodeShape::Trapezoid)
            .unwrap();
        db.add_shaped_node("A", "Asymmetric", crate::core::NodeShape::Asymmetric)
            .unwrap();

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.nodes.len(), 10);
        // All nodes should have valid dimensions
        for node in &result.nodes {
            assert!(node.width > 0);
            assert!(node.height > 0);
        }
    }

    #[test]
    fn test_edge_routing_for_all_directions() {
        let directions = [
            Direction::TopDown,
            Direction::BottomUp,
            Direction::LeftRight,
            Direction::RightLeft,
        ];

        for direction in directions {
            let mut db = FlowchartDatabase::with_direction(direction);
            db.add_simple_node("A", "Start").unwrap();
            db.add_simple_node("B", "End").unwrap();
            db.add_simple_edge("A", "B").unwrap();

            let layout = FlowchartLayoutAlgorithm::new();
            let result = layout.layout(&db).unwrap();

            assert_eq!(result.edges.len(), 1);
            let edge = &result.edges[0];
            assert!(edge.waypoints.len() >= 2);
            // Waypoints should connect from and to nodes
            let from_node = result.nodes.iter().find(|n| n.id == edge.from_id).unwrap();
            let to_node = result.nodes.iter().find(|n| n.id == edge.to_id).unwrap();

            // First waypoint should be near from_node, last near to_node
            let (first_x, first_y) = edge.waypoints[0];
            let (last_x, last_y) = edge.waypoints[edge.waypoints.len() - 1];

            // Check that waypoints are positioned correctly based on direction
            match direction {
                Direction::TopDown => {
                    // First waypoint should be at bottom of from_node
                    assert!(first_y >= from_node.y);
                    // Last waypoint should be at top of to_node
                    assert!(last_y <= to_node.y + to_node.height);
                }
                Direction::BottomUp => {
                    // First waypoint should be at top of from_node
                    assert!(first_y <= from_node.y + from_node.height);
                    // Last waypoint should be at bottom of to_node
                    assert!(last_y >= to_node.y);
                }
                Direction::LeftRight => {
                    // First waypoint should be at right of from_node
                    assert!(first_x >= from_node.x);
                    // Last waypoint should be at left of to_node
                    assert!(last_x <= to_node.x + to_node.width);
                }
                Direction::RightLeft => {
                    // First waypoint should be at left of from_node
                    assert!(first_x <= from_node.x + from_node.width);
                    // Last waypoint should be at right of to_node
                    assert!(last_x >= to_node.x);
                }
            }
        }
    }

    #[test]
    fn test_subgraph_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_simple_node("A", "Start").unwrap();
        db.add_simple_node("B", "Process").unwrap();
        db.add_simple_node("C", "End").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        // Add subgraph containing A and B
        db.add_subgraph("Group".to_string(), vec!["A".to_string(), "B".to_string()]);

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        // Should have one subgraph
        assert_eq!(result.subgraphs.len(), 1);

        let subgraph = &result.subgraphs[0];
        assert_eq!(subgraph.id, "subgraph_0");
        assert_eq!(subgraph.title, "Group");

        // Subgraph should enclose nodes A and B
        let node_a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let node_b = result.nodes.iter().find(|n| n.id == "B").unwrap();

        // Subgraph box should contain both nodes with padding
        assert!(subgraph.x <= node_a.x);
        assert!(subgraph.y <= node_a.y);
        assert!(subgraph.x + subgraph.width >= node_a.x + node_a.width);
        assert!(subgraph.x + subgraph.width >= node_b.x + node_b.width);
        assert!(subgraph.y + subgraph.height >= node_b.y + node_b.height);
    }

    #[test]
    fn test_empty_subgraph_layout() {
        let mut db = FlowchartDatabase::with_direction(Direction::TopDown);

        db.add_simple_node("A", "Start").unwrap();
        db.add_subgraph("Empty".to_string(), vec![]);

        let layout = FlowchartLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        // Should have one subgraph
        assert_eq!(result.subgraphs.len(), 1);

        let subgraph = &result.subgraphs[0];
        assert_eq!(subgraph.title, "Empty");
        assert!(subgraph.width >= 4); // At least title width + borders
        assert!(subgraph.height >= 3); // Minimum height
    }
}
