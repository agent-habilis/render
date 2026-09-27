use std::panic::{self, AssertUnwindSafe};

use anyhow::{Context, Result, anyhow};

/// # Errors
/// The diagram does not parse, or the renderer panics on it.
pub fn mermaid(input: &str) -> Result<String> {
    // figurehead panics rather than erroring on some inputs, so one bad diagram
    // would abort the whole render. Contain it — hook silenced so the backtrace
    // doesn't reach the terminal — letting callers fall back to the raw source.
    // (Only unwinds under a panic=unwind profile; the vendored self-message fix
    // in vendor/figurehead is what keeps discovery.md's diagrams from panicking.)
    let prev = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(|| figurehead::render(input)));
    panic::set_hook(prev);

    match result {
        Ok(render) => render.context("rendering mermaid diagram"),
        Err(_) => Err(anyhow!("mermaid renderer panicked on this diagram")),
    }
}

#[cfg(test)]
mod tests {
    use super::mermaid;

    const DIRECTIONS: [&str; 4] = ["TD", "BT", "LR", "RL"];
    const LINE_CELLS: &str = "─│┌┐└┘├┤┬┴┼▶◀▲▼";

    struct Arrowhead {
        col: isize,
        row: isize,
        // One step toward the side the edge comes from.
        back: (isize, isize),
    }

    fn grid(diagram: &str) -> Vec<Vec<char>> {
        let output = mermaid(diagram).expect("diagram renders");
        output.lines().map(|line| line.chars().collect()).collect()
    }

    fn cell(grid: &[Vec<char>], col: isize, row: isize) -> char {
        usize::try_from(row)
            .ok()
            .and_then(|row| grid.get(row))
            .zip(usize::try_from(col).ok())
            .and_then(|(cells, col)| cells.get(col).copied())
            .unwrap_or(' ')
    }

    fn arrowheads(grid: &[Vec<char>]) -> Vec<Arrowhead> {
        let mut found = Vec::new();
        for (row, cells) in grid.iter().enumerate() {
            for (col, &glyph) in cells.iter().enumerate() {
                let back = match glyph {
                    '▼' => (0, -1),
                    '▲' => (0, 1),
                    '▶' => (-1, 0),
                    '◀' => (1, 0),
                    _ => continue,
                };
                found.push(Arrowhead {
                    col: col.try_into().unwrap(),
                    row: row.try_into().unwrap(),
                    back,
                });
            }
        }
        found
    }

    fn position(grid: &[Vec<char>], target: char) -> (usize, usize) {
        grid.iter()
            .enumerate()
            .find_map(|(row, cells)| {
                cells
                    .iter()
                    .position(|&glyph| glyph == target)
                    .map(|col| (col, row))
            })
            .unwrap_or_else(|| panic!("no {target} in the output"))
    }

    fn split_with_labels(direction: &str) -> String {
        format!(
            "flowchart {direction}\n  a[source] -->|one| b[left]\n  a -->|two| c[middle]\n  a -->|three| d[right]\n"
        )
    }

    fn back_edge_in_subgraph(direction: &str) -> String {
        format!(
            "flowchart {direction}\n  subgraph \"pod\"\n    a[first]\n    b[second]\n    c[third]\n  end\n  a --> b\n  b --> c\n  c --> a\n"
        )
    }

    #[test]
    fn back_edge_between_members_stays_inside_the_subgraph_frame() {
        for direction in DIRECTIONS {
            let diagram = back_edge_in_subgraph(direction);
            let grid = grid(&diagram);
            let (left, top) = position(&grid, '╔');
            let (right, bottom) = position(&grid, '╝');

            for (row, cells) in grid.iter().enumerate().take(bottom).skip(top + 1) {
                assert_eq!(
                    cells[left], '║',
                    "{direction}: left frame broken on row {row}"
                );
                assert_eq!(
                    cells[right], '║',
                    "{direction}: right frame broken on row {row}"
                );
            }
            for (row, cells) in grid.iter().enumerate() {
                for (col, &glyph) in cells.iter().enumerate() {
                    let inside = (left..=right).contains(&col) && (top..=bottom).contains(&row);
                    assert!(
                        inside || !LINE_CELLS.contains(glyph),
                        "{direction}: edge cell {glyph:?} at ({col}, {row}) is outside the frame\n{}",
                        mermaid(&diagram).unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn every_arrowhead_touches_a_line_cell_on_its_incoming_side() {
        for direction in DIRECTIONS {
            let diagram = split_with_labels(direction);
            let grid = grid(&diagram);
            let heads = arrowheads(&grid);
            assert_eq!(heads.len(), 3, "{direction}: expected three arrowheads");
            for Arrowhead {
                col,
                row,
                back: (dx, dy),
            } in heads
            {
                let behind = cell(&grid, col + dx, row + dy);
                let connects = if dx == 0 {
                    "│┌┐└┘├┤┬┴┼"
                } else {
                    "─┌┐└┘├┤┬┴┼"
                };
                assert!(
                    connects.contains(behind),
                    "{direction}: arrowhead at ({col}, {row}) has {behind:?} behind it\n{}",
                    mermaid(&diagram).unwrap()
                );
            }
        }
    }

    #[test]
    fn branch_into_its_target_is_at_least_three_cells_long() {
        for direction in DIRECTIONS {
            let diagram = split_with_labels(direction);
            let grid = grid(&diagram);
            for Arrowhead {
                col,
                row,
                back: (dx, dy),
            } in arrowheads(&grid)
            {
                let straight = if dx == 0 { '│' } else { '─' };
                let mut length = 1;
                while cell(&grid, col + dx * length, row + dy * length) == straight {
                    length += 1;
                }
                assert!(
                    length >= 3,
                    "{direction}: branch into arrowhead at ({col}, {row}) is {length} cells long\n{}",
                    mermaid(&diagram).unwrap()
                );
            }
        }
    }

    #[test]
    fn every_arrowhead_touches_its_target_box() {
        for direction in DIRECTIONS {
            for diagram in [
                split_with_labels(direction),
                back_edge_in_subgraph(direction),
            ] {
                let grid = grid(&diagram);
                for Arrowhead {
                    col,
                    row,
                    back: (dx, dy),
                } in arrowheads(&grid)
                {
                    assert_ne!(
                        cell(&grid, col - dx, row - dy),
                        ' ',
                        "{direction}: arrowhead at ({col}, {row}) does not touch its target\n{}",
                        mermaid(&diagram).unwrap()
                    );
                }
            }
        }
    }
}
