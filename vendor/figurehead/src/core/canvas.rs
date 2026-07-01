//! Shared ASCII canvas for all diagram renderers
//!
//! Provides a common grid-based canvas that can be used by any plugin renderer.

/// ASCII canvas representing a character grid for diagram rendering
#[derive(Debug, Clone)]
pub struct AsciiCanvas {
    pub width: usize,
    pub height: usize,
    pub grid: Vec<Vec<char>>,
}

impl AsciiCanvas {
    /// Create a new canvas with the specified dimensions
    pub fn new(width: usize, height: usize) -> Self {
        let grid = vec![vec![' '; width.max(1)]; height.max(1)];
        Self {
            width,
            height,
            grid,
        }
    }

    /// Ensure the canvas is at least the specified size, expanding if needed
    pub fn ensure_size(&mut self, min_width: usize, min_height: usize) {
        if min_width > self.width {
            for row in &mut self.grid {
                row.resize(min_width, ' ');
            }
            self.width = min_width;
        }
        if min_height > self.height {
            let extra_rows = min_height - self.height;
            self.grid
                .extend((0..extra_rows).map(|_| vec![' '; self.width]));
            self.height = min_height;
        }
    }

    /// Set a character at the specified position
    pub fn set_char(&mut self, x: usize, y: usize, c: char) {
        self.ensure_size(x + 1, y + 1);
        self.grid[y][x] = c;
    }

    /// Get the character at the specified position
    pub fn get_char(&self, x: usize, y: usize) -> char {
        if y < self.height && x < self.width {
            self.grid[y][x]
        } else {
            ' '
        }
    }

    /// Draw text at the specified position (left-aligned)
    pub fn draw_text(&mut self, x: usize, y: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let char_count = text.chars().count();
        self.ensure_size(x + char_count, y + 1);
        for (i, c) in text.chars().enumerate() {
            self.set_char(x + i, y, c);
        }
    }

    /// Draw text centered at the specified x position
    pub fn draw_text_centered(&mut self, center_x: usize, y: usize, text: &str) {
        let char_count = text.chars().count();
        let start_x = center_x.saturating_sub(char_count / 2);
        self.draw_text(start_x, y, text);
    }

    /// Draw a horizontal line
    pub fn draw_horizontal_line(&mut self, x: usize, y: usize, length: usize, c: char) {
        for i in 0..length {
            self.set_char(x + i, y, c);
        }
    }

    /// Draw a vertical line
    pub fn draw_vertical_line(&mut self, x: usize, y: usize, length: usize, c: char) {
        for i in 0..length {
            self.set_char(x, y + i, c);
        }
    }
}

impl std::fmt::Display for AsciiCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut rows: Vec<String> = self
            .grid
            .iter()
            .map(|row| {
                let s: String = row.iter().collect();
                s.trim_end().to_string()
            })
            .collect();

        // Trim empty rows from top and bottom
        while rows.first().is_some_and(|row| row.is_empty()) {
            rows.remove(0);
        }
        while rows.last().is_some_and(|row| row.is_empty()) {
            rows.pop();
        }

        if rows.is_empty() {
            return Ok(());
        }

        // Remove common leading whitespace
        let min_indent = rows
            .iter()
            .filter(|row| !row.is_empty())
            .map(|row| row.chars().take_while(|c| *c == ' ').count())
            .min()
            .unwrap_or(0);

        if min_indent > 0 {
            for row in &mut rows {
                *row = row.chars().skip(min_indent).collect();
            }
        }

        write!(f, "{}", rows.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_canvas() {
        let canvas = AsciiCanvas::new(10, 5);
        assert_eq!(canvas.width, 10);
        assert_eq!(canvas.height, 5);
    }

    #[test]
    fn test_set_and_get_char() {
        let mut canvas = AsciiCanvas::new(10, 10);
        canvas.set_char(5, 3, 'X');
        assert_eq!(canvas.get_char(5, 3), 'X');
        assert_eq!(canvas.get_char(0, 0), ' ');
    }

    #[test]
    fn test_auto_expand() {
        let mut canvas = AsciiCanvas::new(5, 5);
        canvas.set_char(10, 10, 'X');
        assert!(canvas.width >= 11);
        assert!(canvas.height >= 11);
        assert_eq!(canvas.get_char(10, 10), 'X');
    }

    #[test]
    fn test_draw_text() {
        let mut canvas = AsciiCanvas::new(20, 5);
        canvas.draw_text(2, 1, "Hello");
        assert_eq!(canvas.get_char(2, 1), 'H');
        assert_eq!(canvas.get_char(6, 1), 'o');
    }

    #[test]
    fn test_draw_text_centered() {
        let mut canvas = AsciiCanvas::new(20, 5);
        canvas.draw_text_centered(10, 1, "Hi");
        // "Hi" has 2 chars, center at 10 means starts at 9
        assert_eq!(canvas.get_char(9, 1), 'H');
        assert_eq!(canvas.get_char(10, 1), 'i');
    }

    #[test]
    fn test_draw_lines() {
        let mut canvas = AsciiCanvas::new(10, 10);
        canvas.draw_horizontal_line(2, 3, 5, '-');
        canvas.draw_vertical_line(4, 1, 4, '|');

        assert_eq!(canvas.get_char(2, 3), '-');
        assert_eq!(canvas.get_char(6, 3), '-');
        assert_eq!(canvas.get_char(4, 1), '|');
        assert_eq!(canvas.get_char(4, 4), '|');
    }

    #[test]
    fn test_display_trims_whitespace() {
        let mut canvas = AsciiCanvas::new(20, 10);
        canvas.draw_text(5, 3, "Test");
        let output = canvas.to_string();
        assert_eq!(output, "Test");
    }
}
