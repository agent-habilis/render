//! Shared edge routing for diagram renderers
//!
//! Provides common edge routing algorithms: straight lines, orthogonal paths,
//! split edges (one-to-many), and merge edges (many-to-one).

use super::{AsciiCanvas, CharacterSet, Direction};

/// Character set for edge drawing
#[derive(Debug, Clone, Copy)]
pub struct EdgeChars {
    pub horizontal: char,
    pub vertical: char,
    pub corner_top_left: char,     // ┌ (goes RIGHT and DOWN)
    pub corner_top_right: char,    // ┐ (goes LEFT and DOWN)
    pub corner_bottom_left: char,  // └ (goes RIGHT and UP)
    pub corner_bottom_right: char, // ┘ (goes LEFT and UP)
    pub junction_down: char,       // ┬ (connects LEFT, RIGHT, DOWN - line from above splits)
    pub junction_up: char,         // ┴ (connects LEFT, RIGHT, UP - line from below splits)
    pub junction_right: char,      // ├ (connects UP, DOWN, RIGHT)
    pub junction_left: char,       // ┤ (connects UP, DOWN, LEFT)
    pub cross: char,               // ┼ (all four directions)
    pub arrow_up: char,
    pub arrow_down: char,
    pub arrow_left: char,
    pub arrow_right: char,
}

impl EdgeChars {
    /// Get edge characters for the given style
    pub fn for_style(style: CharacterSet) -> Self {
        if style.is_ascii() {
            Self::ascii()
        } else {
            Self::unicode()
        }
    }

    /// ASCII edge characters
    pub fn ascii() -> Self {
        Self {
            horizontal: '-',
            vertical: '|',
            corner_top_left: '+',
            corner_top_right: '+',
            corner_bottom_left: '+',
            corner_bottom_right: '+',
            junction_down: '+',
            junction_up: '+',
            junction_right: '+',
            junction_left: '+',
            cross: '+',
            arrow_up: '^',
            arrow_down: 'v',
            arrow_left: '<',
            arrow_right: '>',
        }
    }

    /// Unicode box-drawing edge characters
    pub fn unicode() -> Self {
        Self {
            horizontal: '─',
            vertical: '│',
            corner_top_left: '┌',
            corner_top_right: '┐',
            corner_bottom_left: '└',
            corner_bottom_right: '┘',
            junction_down: '┬',
            junction_up: '┴',
            junction_right: '├',
            junction_left: '┤',
            cross: '┼',
            arrow_up: '▲',
            arrow_down: '▼',
            arrow_left: '◀',
            arrow_right: '▶',
        }
    }
}

/// Edge routing helper for diagram renderers
pub struct EdgeRouter {
    pub chars: EdgeChars,
}

impl EdgeRouter {
    /// Create a new edge router with the given character set
    pub fn new(style: CharacterSet) -> Self {
        Self {
            chars: EdgeChars::for_style(style),
        }
    }

    /// Create an edge router with custom characters
    pub fn with_chars(chars: EdgeChars) -> Self {
        Self { chars }
    }

    /// Draw a horizontal line from x1 to x2 at y
    pub fn draw_horizontal(&self, canvas: &mut AsciiCanvas, y: usize, x1: usize, x2: usize) {
        let (start, end) = (x1.min(x2), x1.max(x2));
        for x in start..=end {
            canvas.set_char(x, y, self.chars.horizontal);
        }
    }

    /// Draw a vertical line from y1 to y2 at x
    pub fn draw_vertical(&self, canvas: &mut AsciiCanvas, x: usize, y1: usize, y2: usize) {
        let (start, end) = (y1.min(y2), y1.max(y2));
        for y in start..=end {
            canvas.set_char(x, y, self.chars.vertical);
        }
    }

    /// Draw an arrow at the given position
    pub fn draw_arrow(&self, canvas: &mut AsciiCanvas, x: usize, y: usize, direction: Direction) {
        let arrow = match direction {
            Direction::TopDown => self.chars.arrow_down,
            Direction::BottomUp => self.chars.arrow_up,
            Direction::LeftRight => self.chars.arrow_right,
            Direction::RightLeft => self.chars.arrow_left,
        };
        canvas.set_char(x, y, arrow);
    }

    /// Draw a straight edge from source to target (vertical in TD/BT, horizontal in LR/RL)
    pub fn draw_straight_edge(
        &self,
        canvas: &mut AsciiCanvas,
        from: (usize, usize),
        to: (usize, usize),
        direction: Direction,
        with_arrow: bool,
    ) {
        let (from_x, from_y) = from;
        let (to_x, to_y) = to;

        match direction {
            Direction::TopDown | Direction::BottomUp => {
                // Draw vertical line
                let (start_y, end_y) = if from_y < to_y {
                    (
                        from_y,
                        if with_arrow {
                            to_y.saturating_sub(1)
                        } else {
                            to_y
                        },
                    )
                } else {
                    (if with_arrow { to_y + 1 } else { to_y }, from_y)
                };
                self.draw_vertical(canvas, from_x, start_y, end_y);
                if with_arrow {
                    let arrow_dir = if from_y < to_y {
                        Direction::TopDown
                    } else {
                        Direction::BottomUp
                    };
                    self.draw_arrow(canvas, to_x, end_y, arrow_dir);
                }
            }
            Direction::LeftRight | Direction::RightLeft => {
                // Draw horizontal line
                let (start_x, end_x) = if from_x < to_x {
                    (
                        from_x,
                        if with_arrow {
                            to_x.saturating_sub(1)
                        } else {
                            to_x
                        },
                    )
                } else {
                    (if with_arrow { to_x + 1 } else { to_x }, from_x)
                };
                self.draw_horizontal(canvas, from_y, start_x, end_x);
                if with_arrow {
                    let arrow_dir = if from_x < to_x {
                        Direction::LeftRight
                    } else {
                        Direction::RightLeft
                    };
                    self.draw_arrow(canvas, end_x, to_y, arrow_dir);
                }
            }
        }
    }

    /// Draw split edges: one source to multiple targets (TopDown)
    ///
    /// Layout:
    /// ```text
    ///     │        <- vertical from source
    ///     ┴        <- junction_up (splits LEFT and RIGHT)
    /// ┌───┴───┐    <- horizontal bar
    /// ▼       ▼    <- arrows to targets
    /// ```
    pub fn draw_split_edges_td(
        &self,
        canvas: &mut AsciiCanvas,
        from_x: usize,
        from_y: usize,
        targets: &[(usize, usize)],
        with_arrows: bool,
    ) {
        if targets.is_empty() {
            return;
        }

        if targets.len() == 1 {
            // Single target - just draw straight line
            self.draw_straight_edge(
                canvas,
                (from_x, from_y),
                targets[0],
                Direction::TopDown,
                with_arrows,
            );
            return;
        }

        // Find the horizontal span for the bar
        let min_x = targets.iter().map(|(x, _)| *x).min().unwrap();
        let max_x = targets.iter().map(|(x, _)| *x).max().unwrap();

        // Junction Y is 1 below source
        let junction_y = from_y + 1;

        // Draw vertical from source to junction
        self.draw_vertical(canvas, from_x, from_y, junction_y);

        // Draw horizontal bar
        self.draw_horizontal(canvas, junction_y, min_x, max_x);

        // Draw junction character at source position
        let junction_char = if from_x <= min_x {
            self.chars.corner_bottom_left // └
        } else if from_x >= max_x {
            self.chars.corner_bottom_right // ┘
        } else {
            self.chars.junction_up // ┴ (connects UP, LEFT, RIGHT)
        };
        canvas.set_char(from_x, junction_y, junction_char);

        // Draw corners and vertical lines to each target
        for &(tx, ty) in targets {
            if tx == from_x {
                // Target is directly below source - continue the line below junction
                let end_y = if with_arrows {
                    ty.saturating_sub(1)
                } else {
                    ty
                };
                if junction_y < end_y {
                    self.draw_vertical(canvas, tx, junction_y + 1, end_y);
                }
            } else {
                // Draw corner at target x position
                let corner = if tx < from_x {
                    self.chars.corner_top_left // ┌
                } else {
                    self.chars.corner_top_right // ┐
                };
                canvas.set_char(tx, junction_y, corner);

                // Draw vertical line to target
                let end_y = if with_arrows {
                    ty.saturating_sub(1)
                } else {
                    ty
                };
                self.draw_vertical(canvas, tx, junction_y + 1, end_y);
            }

            // Draw arrow if requested
            if with_arrows {
                self.draw_arrow(canvas, tx, ty.saturating_sub(1), Direction::TopDown);
            }
        }
    }

    /// Draw merge edges: multiple sources to one target (TopDown)
    ///
    /// Layout:
    /// ```text
    /// │       │    <- verticals from sources
    /// └───┬───┘    <- horizontal bar with junction_down
    ///     │        <- vertical to target
    ///     ▼        <- arrow
    /// ```
    pub fn draw_merge_edges_td(
        &self,
        canvas: &mut AsciiCanvas,
        sources: &[(usize, usize)],
        to_x: usize,
        to_y: usize,
        with_arrow: bool,
    ) {
        if sources.is_empty() {
            return;
        }

        if sources.len() == 1 {
            // Single source - just draw straight line
            self.draw_straight_edge(
                canvas,
                sources[0],
                (to_x, to_y),
                Direction::TopDown,
                with_arrow,
            );
            return;
        }

        // Find the horizontal span and lowest source
        let min_x = sources.iter().map(|(x, _)| *x).min().unwrap();
        let max_x = sources.iter().map(|(x, _)| *x).max().unwrap();
        let max_source_y = sources.iter().map(|(_, y)| *y).max().unwrap();

        // Junction Y is 1 below the lowest source
        let junction_y = max_source_y + 1;

        // Draw vertical from each source to junction line
        for &(sx, sy) in sources {
            self.draw_vertical(canvas, sx, sy, junction_y);
        }

        // Draw horizontal bar
        self.draw_horizontal(canvas, junction_y, min_x, max_x);

        // Draw corners at source positions
        for &(sx, _) in sources {
            let corner = if sx < to_x {
                self.chars.corner_bottom_left // └
            } else if sx > to_x {
                self.chars.corner_bottom_right // ┘
            } else {
                self.chars.junction_down // ┬
            };
            canvas.set_char(sx, junction_y, corner);
        }

        // Draw junction at target x if not at a source
        if !sources.iter().any(|(x, _)| *x == to_x) {
            canvas.set_char(to_x, junction_y, self.chars.junction_down);
        }

        // Draw vertical from junction to target (starting BELOW junction to avoid overwriting)
        let end_y = if with_arrow {
            to_y.saturating_sub(1)
        } else {
            to_y
        };
        if junction_y < end_y {
            self.draw_vertical(canvas, to_x, junction_y + 1, end_y);
        }

        // Draw arrow
        if with_arrow {
            self.draw_arrow(canvas, to_x, end_y, Direction::TopDown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_chars_unicode() {
        let chars = EdgeChars::unicode();
        assert_eq!(chars.horizontal, '─');
        assert_eq!(chars.junction_down, '┬');
        assert_eq!(chars.junction_up, '┴');
    }

    #[test]
    fn test_edge_chars_ascii() {
        let chars = EdgeChars::ascii();
        assert_eq!(chars.horizontal, '-');
        assert_eq!(chars.junction_down, '+');
    }

    #[test]
    fn test_draw_horizontal() {
        let router = EdgeRouter::new(CharacterSet::Unicode);
        let mut canvas = AsciiCanvas::new(20, 5);
        router.draw_horizontal(&mut canvas, 2, 5, 10);

        for x in 5..=10 {
            assert_eq!(canvas.get_char(x, 2), '─');
        }
    }

    #[test]
    fn test_draw_vertical() {
        let router = EdgeRouter::new(CharacterSet::Unicode);
        let mut canvas = AsciiCanvas::new(10, 10);
        router.draw_vertical(&mut canvas, 5, 2, 7);

        for y in 2..=7 {
            assert_eq!(canvas.get_char(5, y), '│');
        }
    }

    #[test]
    fn test_draw_split_edges() {
        let router = EdgeRouter::new(CharacterSet::Unicode);
        let mut canvas = AsciiCanvas::new(20, 10);

        // Source at (10, 2), targets at (5, 6) and (15, 6)
        router.draw_split_edges_td(&mut canvas, 10, 2, &[(5, 6), (15, 6)], true);

        // Should have junction at (10, 3)
        assert_eq!(canvas.get_char(10, 3), '┴');
        // Corners at target x positions
        assert_eq!(canvas.get_char(5, 3), '┌');
        assert_eq!(canvas.get_char(15, 3), '┐');
    }

    #[test]
    fn test_draw_merge_edges() {
        let router = EdgeRouter::new(CharacterSet::Unicode);
        let mut canvas = AsciiCanvas::new(20, 10);

        // Sources at (5, 2) and (15, 2), target at (10, 8)
        router.draw_merge_edges_td(&mut canvas, &[(5, 2), (15, 2)], 10, 8, true);

        // Junction at (10, 3)
        assert_eq!(canvas.get_char(10, 3), '┬');
        // Corners at source x positions
        assert_eq!(canvas.get_char(5, 3), '└');
        assert_eq!(canvas.get_char(15, 3), '┘');
    }
}
