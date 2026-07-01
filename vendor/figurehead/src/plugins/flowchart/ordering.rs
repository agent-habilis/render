//! Barycenter ordering algorithm for edge crossing minimization
//!
//! Implements the ordering phase of the Sugiyama layout algorithm,
//! using barycenter heuristics to minimize edge crossings.

use std::collections::HashMap;

use super::FlowchartDatabase;

/// Count edge crossings between all adjacent layers.
///
/// An edge crossing occurs when two edges between adjacent layers
/// intersect. For edges (a1→b1) and (a2→b2) where a1, a2 are in layer L
/// and b1, b2 are in layer L+1, they cross if:
/// - a1 is left of a2 (pos(a1) < pos(a2))
/// - b1 is right of b2 (pos(b1) > pos(b2))
///
/// Or vice versa.
pub fn cross_count(layers: &[Vec<&str>], db: &FlowchartDatabase) -> usize {
    let mut total = 0;
    for i in 0..layers.len().saturating_sub(1) {
        total += two_layer_cross_count(&layers[i], &layers[i + 1], db);
    }
    total
}

/// Count crossings between two adjacent layers.
fn two_layer_cross_count(north: &[&str], south: &[&str], db: &FlowchartDatabase) -> usize {
    // Build position maps
    let north_pos: HashMap<&str, usize> = north.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let south_pos: HashMap<&str, usize> = south.iter().enumerate().map(|(i, &n)| (n, i)).collect();

    // Collect all edges between these layers as (north_pos, south_pos) pairs
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for &n in north {
        for succ in db.successors(n) {
            if let Some(&sp) = south_pos.get(succ) {
                if let Some(&np) = north_pos.get(n) {
                    edges.push((np, sp));
                }
            }
        }
    }

    // Count crossings: O(E²) simple version
    // Two edges (n1, s1) and (n2, s2) cross if (n1 < n2 && s1 > s2) || (n1 > n2 && s1 < s2)
    let mut crossings = 0;
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            let (n1, s1) = edges[i];
            let (n2, s2) = edges[j];
            if (n1 < n2 && s1 > s2) || (n1 > n2 && s1 < s2) {
                crossings += 1;
            }
        }
    }
    crossings
}

/// Direction for barycenter calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepDirection {
    /// Look at predecessors (nodes in previous layer)
    Downward,
    /// Look at successors (nodes in next layer)
    Upward,
}

/// Compute barycenter values for nodes in a layer.
///
/// The barycenter of a node is the average position of its neighbors
/// in the reference layer. Returns None for nodes with no connections.
pub fn compute_barycenters(
    layer: &[&str],
    ref_layer: &[&str],
    db: &FlowchartDatabase,
    direction: SweepDirection,
) -> Vec<Option<f64>> {
    // Build position map for reference layer
    let ref_pos: HashMap<&str, usize> =
        ref_layer.iter().enumerate().map(|(i, &n)| (n, i)).collect();

    layer
        .iter()
        .map(|&node| {
            // Get neighbors based on direction
            let neighbors: Vec<&str> = match direction {
                SweepDirection::Downward => db.predecessors(node),
                SweepDirection::Upward => db.successors(node),
            };

            // Filter to neighbors in reference layer and get their positions
            let positions: Vec<f64> = neighbors
                .iter()
                .filter_map(|&n| ref_pos.get(n).map(|&p| p as f64))
                .collect();

            if positions.is_empty() {
                None
            } else {
                Some(positions.iter().sum::<f64>() / positions.len() as f64)
            }
        })
        .collect()
}

/// Order nodes in a layer by their barycenter values.
///
/// Nodes with barycenters are sorted by their barycenter value.
/// Nodes without barycenters (None) keep their relative positions
/// among other None nodes, interspersed at their original indices.
pub fn order_layer_by_barycenter(layer: &mut Vec<&str>, barycenters: &[Option<f64>]) {
    // Create (node, barycenter, original_index) tuples
    let mut entries: Vec<(&str, Option<f64>, usize)> = layer
        .iter()
        .enumerate()
        .map(|(i, &node)| (node, barycenters.get(i).copied().flatten(), i))
        .collect();

    // Stable sort by barycenter, with None values using their original index
    // to maintain relative order among unconnected nodes
    entries.sort_by(|a, b| {
        match (&a.1, &b.1) {
            (Some(bc_a), Some(bc_b)) => {
                // Both have barycenters - sort by barycenter, then original index for ties
                bc_a.partial_cmp(bc_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.2.cmp(&b.2))
            }
            (Some(_), None) => std::cmp::Ordering::Less, // Nodes with barycenter come first
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.2.cmp(&b.2), // Both None - keep original order
        }
    });

    // Extract sorted nodes back into layer
    *layer = entries.into_iter().map(|(node, _, _)| node).collect();
}

/// Apply barycenter ordering to minimize edge crossings.
///
/// Performs multiple sweeps (alternating downward/upward) to iteratively
/// improve the ordering. Keeps track of the best ordering found.
///
/// Returns the crossing count of the best ordering found.
pub fn order_layers_barycenter(
    db: &FlowchartDatabase,
    layers: &mut Vec<Vec<&str>>,
    iterations: usize,
) -> usize {
    if layers.len() < 2 {
        return 0; // No crossings possible with 0 or 1 layers
    }

    let mut best_layers = layers.clone();
    let mut best_cc = cross_count(layers, db);

    for i in 0..iterations {
        let downward = i % 2 == 0;

        // Determine layer indices to process
        let layer_indices: Vec<usize> = if downward {
            (1..layers.len()).collect()
        } else {
            (0..layers.len() - 1).rev().collect()
        };

        for layer_idx in layer_indices {
            let ref_idx = if downward {
                layer_idx - 1
            } else {
                layer_idx + 1
            };
            let direction = if downward {
                SweepDirection::Downward
            } else {
                SweepDirection::Upward
            };

            // Compute barycenters and reorder
            let barycenters =
                compute_barycenters(&layers[layer_idx], &layers[ref_idx], db, direction);
            order_layer_by_barycenter(&mut layers[layer_idx], &barycenters);
        }

        // Check if this ordering is better
        let cc = cross_count(layers, db);
        if cc < best_cc {
            best_layers = layers.clone();
            best_cc = cc;
        }
    }

    // Apply best ordering found
    *layers = best_layers;
    best_cc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Direction;

    fn create_db() -> FlowchartDatabase {
        FlowchartDatabase::with_direction(Direction::TopDown)
    }

    // =========================================================================
    // Phase 1: Cross Count Tests
    // =========================================================================

    #[test]
    fn test_cross_count_no_crossings() {
        // Layer 0: [A, B]
        // Layer 1: [C, D]
        // Edges: A→C, B→D (parallel, no crossing)
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();

        let layers = vec![vec!["A", "B"], vec!["C", "D"]];
        assert_eq!(cross_count(&layers, &db), 0);
    }

    #[test]
    fn test_cross_count_one_crossing() {
        // Layer 0: [A, B]
        // Layer 1: [C, D]
        // Edges: A→D, B→C (X pattern, 1 crossing)
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        let layers = vec![vec!["A", "B"], vec!["C", "D"]];
        assert_eq!(cross_count(&layers, &db), 1);
    }

    #[test]
    fn test_cross_count_multi_layer() {
        // Layer 0: [A, B]
        // Layer 1: [C, D]
        // Layer 2: [E, F]
        // Edges: A→D, B→C (1 crossing), C→F, D→E (1 crossing)
        // Total: 2 crossings
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_node("E", "E").unwrap();
        db.add_simple_node("F", "F").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("B", "C").unwrap();
        db.add_simple_edge("C", "F").unwrap();
        db.add_simple_edge("D", "E").unwrap();

        let layers = vec![vec!["A", "B"], vec!["C", "D"], vec!["E", "F"]];
        assert_eq!(cross_count(&layers, &db), 2);
    }

    #[test]
    fn test_cross_count_empty_layers() {
        let db = create_db();
        let layers: Vec<Vec<&str>> = vec![];
        assert_eq!(cross_count(&layers, &db), 0);
    }

    #[test]
    fn test_cross_count_single_layer() {
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        let layers = vec![vec!["A"]];
        assert_eq!(cross_count(&layers, &db), 0);
    }

    // =========================================================================
    // Phase 2: Barycenter Calculation Tests
    // =========================================================================

    #[test]
    fn test_barycenter_no_predecessors() {
        // Node in layer 1 with no edges from layer 0
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        // No edges

        let layer = vec!["B"];
        let ref_layer = vec!["A"];
        let bcs = compute_barycenters(&layer, &ref_layer, &db, SweepDirection::Downward);

        assert_eq!(bcs.len(), 1);
        assert_eq!(bcs[0], None);
    }

    #[test]
    fn test_barycenter_one_predecessor() {
        // Layer 0: [A] at position 0
        // Layer 1: [B] with edge A→B
        // Expected: barycenter(B) = 0.0
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_edge("A", "B").unwrap();

        let layer = vec!["B"];
        let ref_layer = vec!["A"];
        let bcs = compute_barycenters(&layer, &ref_layer, &db, SweepDirection::Downward);

        assert_eq!(bcs.len(), 1);
        assert_eq!(bcs[0], Some(0.0));
    }

    #[test]
    fn test_barycenter_multiple_predecessors() {
        // Layer 0: [A, B, C] at positions 0, 1, 2
        // Layer 1: [D] with edges A→D, C→D
        // Expected: barycenter(D) = (0 + 2) / 2 = 1.0
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("C", "D").unwrap();

        let layer = vec!["D"];
        let ref_layer = vec!["A", "B", "C"];
        let bcs = compute_barycenters(&layer, &ref_layer, &db, SweepDirection::Downward);

        assert_eq!(bcs.len(), 1);
        assert_eq!(bcs[0], Some(1.0));
    }

    #[test]
    fn test_barycenter_upward_direction() {
        // Layer 0: [A] with edge A→B
        // Layer 1: [B] at position 0
        // Upward sweep: barycenter of A based on successors
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_edge("A", "B").unwrap();

        let layer = vec!["A"];
        let ref_layer = vec!["B"];
        let bcs = compute_barycenters(&layer, &ref_layer, &db, SweepDirection::Upward);

        assert_eq!(bcs.len(), 1);
        assert_eq!(bcs[0], Some(0.0));
    }

    #[test]
    fn test_barycenter_multiple_nodes() {
        // Layer 0: [A, B] at positions 0, 1
        // Layer 1: [C, D]
        // Edges: A→C, B→D
        // barycenter(C) = 0.0, barycenter(D) = 1.0
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();

        let layer = vec!["C", "D"];
        let ref_layer = vec!["A", "B"];
        let bcs = compute_barycenters(&layer, &ref_layer, &db, SweepDirection::Downward);

        assert_eq!(bcs.len(), 2);
        assert_eq!(bcs[0], Some(0.0)); // C's barycenter
        assert_eq!(bcs[1], Some(1.0)); // D's barycenter
    }

    #[test]
    fn test_barycenter_mixed_connectivity() {
        // Layer 0: [A, B, C]
        // Layer 1: [D, E, F]
        // Edges: A→D, A→E, C→F (B has no outgoing to layer 1)
        // D: bc=0, E: bc=0, F: bc=2
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_node("E", "E").unwrap();
        db.add_simple_node("F", "F").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("A", "E").unwrap();
        db.add_simple_edge("C", "F").unwrap();

        let layer = vec!["D", "E", "F"];
        let ref_layer = vec!["A", "B", "C"];
        let bcs = compute_barycenters(&layer, &ref_layer, &db, SweepDirection::Downward);

        assert_eq!(bcs.len(), 3);
        assert_eq!(bcs[0], Some(0.0)); // D
        assert_eq!(bcs[1], Some(0.0)); // E
        assert_eq!(bcs[2], Some(2.0)); // F
    }

    // =========================================================================
    // Phase 3: Layer Ordering Tests
    // =========================================================================

    #[test]
    fn test_order_by_barycenter_simple() {
        // Nodes [A, B, C] with barycenters [2.0, 0.5, 1.0]
        // Expected order: [B, C, A]
        let mut layer = vec!["A", "B", "C"];
        let barycenters = vec![Some(2.0), Some(0.5), Some(1.0)];

        order_layer_by_barycenter(&mut layer, &barycenters);

        assert_eq!(layer, vec!["B", "C", "A"]);
    }

    #[test]
    fn test_order_by_barycenter_tie_breaking() {
        // Nodes [A, B] with barycenters [1.0, 1.0]
        // Expected: [A, B] (preserve original order on ties)
        let mut layer = vec!["A", "B"];
        let barycenters = vec![Some(1.0), Some(1.0)];

        order_layer_by_barycenter(&mut layer, &barycenters);

        assert_eq!(layer, vec!["A", "B"]);
    }

    #[test]
    fn test_order_by_barycenter_with_none() {
        // [A, B, C] where B has no barycenter
        // A.bc=2.0, B.bc=None, C.bc=0.0
        // Nodes with barycenters first, then None nodes
        let mut layer = vec!["A", "B", "C"];
        let barycenters = vec![Some(2.0), None, Some(0.0)];

        order_layer_by_barycenter(&mut layer, &barycenters);

        // C (0.0) < A (2.0), then B (None) at end
        assert_eq!(layer, vec!["C", "A", "B"]);
    }

    #[test]
    fn test_order_by_barycenter_all_none() {
        // All nodes have no barycenter - keep original order
        let mut layer = vec!["A", "B", "C"];
        let barycenters = vec![None, None, None];

        order_layer_by_barycenter(&mut layer, &barycenters);

        assert_eq!(layer, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_order_by_barycenter_multiple_none() {
        // [A, B, C, D] where A, C have barycenters, B, D don't
        // A.bc=1.0, B.bc=None, C.bc=0.0, D.bc=None
        let mut layer = vec!["A", "B", "C", "D"];
        let barycenters = vec![Some(1.0), None, Some(0.0), None];

        order_layer_by_barycenter(&mut layer, &barycenters);

        // C (0.0) < A (1.0), then B, D (None, in original relative order)
        assert_eq!(layer, vec!["C", "A", "B", "D"]);
    }

    #[test]
    fn test_order_by_barycenter_empty() {
        let mut layer: Vec<&str> = vec![];
        let barycenters: Vec<Option<f64>> = vec![];

        order_layer_by_barycenter(&mut layer, &barycenters);

        assert!(layer.is_empty());
    }

    #[test]
    fn test_order_by_barycenter_single() {
        let mut layer = vec!["A"];
        let barycenters = vec![Some(1.0)];

        order_layer_by_barycenter(&mut layer, &barycenters);

        assert_eq!(layer, vec!["A"]);
    }

    // =========================================================================
    // Phase 4: Full Ordering Algorithm Tests
    // =========================================================================

    #[test]
    fn test_order_layers_fixes_crossing() {
        // Two-layer graph with crossing:
        // Layer 0: [A, B]
        // Layer 1: [D, C]  <- wrong order causes crossing
        // Edges: A→C, B→D
        // A(0)→C(1) and B(1)→D(0) cross!
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();

        // Start with crossing order
        let mut layers = vec![vec!["A", "B"], vec!["D", "C"]];
        let initial_cc = cross_count(&layers, &db);
        assert_eq!(initial_cc, 1); // A→C and B→D cross

        let final_cc = order_layers_barycenter(&db, &mut layers, 4);

        // Should have 0 crossings after ordering
        assert_eq!(final_cc, 0);
        // Layer 1 should be reordered to [C, D]
        assert_eq!(layers[1], vec!["C", "D"]);
    }

    #[test]
    fn test_order_layers_already_optimal() {
        // Already optimal ordering - should stay the same
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();

        let mut layers = vec![vec!["A", "B"], vec!["C", "D"]];
        let initial_cc = cross_count(&layers, &db);
        assert_eq!(initial_cc, 0);

        let final_cc = order_layers_barycenter(&db, &mut layers, 4);

        assert_eq!(final_cc, 0);
    }

    #[test]
    fn test_order_layers_deterministic() {
        // Same input should produce same output
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("B", "C").unwrap();

        let mut layers1 = vec![vec!["A", "B"], vec!["C", "D"]];
        let mut layers2 = vec![vec!["A", "B"], vec!["C", "D"]];

        let cc1 = order_layers_barycenter(&db, &mut layers1, 4);
        let cc2 = order_layers_barycenter(&db, &mut layers2, 4);

        assert_eq!(cc1, cc2);
        assert_eq!(layers1, layers2);
    }

    #[test]
    fn test_order_layers_empty() {
        let db = create_db();
        let mut layers: Vec<Vec<&str>> = vec![];

        let cc = order_layers_barycenter(&db, &mut layers, 4);

        assert_eq!(cc, 0);
    }

    #[test]
    fn test_order_layers_single_layer() {
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();

        let mut layers = vec![vec!["A", "B"]];

        let cc = order_layers_barycenter(&db, &mut layers, 4);

        assert_eq!(cc, 0);
    }

    #[test]
    fn test_order_layers_complex_improves() {
        // Complex graph where ordering should improve crossings
        //   A   B
        //   |\ /|
        //   | X |   <- crossings
        //   |/ \|
        //   C   D
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("A", "D").unwrap();
        db.add_simple_edge("B", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();

        // With A, B on top and D, C on bottom - should have crossings
        let mut layers = vec![vec!["A", "B"], vec!["D", "C"]];
        let initial_cc = cross_count(&layers, &db);

        let final_cc = order_layers_barycenter(&db, &mut layers, 4);

        // Final should be <= initial (barycenter may not always find optimal)
        assert!(final_cc <= initial_cc);
    }

    #[test]
    fn test_order_layers_three_layers() {
        // Three layer graph
        //   A
        //  / \
        // B   C
        //  \ /
        //   D
        let mut db = create_db();
        db.add_simple_node("A", "A").unwrap();
        db.add_simple_node("B", "B").unwrap();
        db.add_simple_node("C", "C").unwrap();
        db.add_simple_node("D", "D").unwrap();
        db.add_simple_edge("A", "B").unwrap();
        db.add_simple_edge("A", "C").unwrap();
        db.add_simple_edge("B", "D").unwrap();
        db.add_simple_edge("C", "D").unwrap();

        // Start with potentially suboptimal order
        let mut layers = vec![vec!["A"], vec!["C", "B"], vec!["D"]];

        let final_cc = order_layers_barycenter(&db, &mut layers, 4);

        // Should achieve 0 crossings for diamond
        assert_eq!(final_cc, 0);
    }
}
