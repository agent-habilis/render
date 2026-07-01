//! Shared box drawing characters for diagram rendering
//!
//! This module provides consistent box drawing characters across all renderers,
//! supporting both ASCII and Unicode modes.

use super::CharacterSet;

/// Box drawing characters for rendering rectangular shapes
#[derive(Debug, Clone, Copy)]
pub struct BoxChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    /// T-junction pointing right (for left edge separators)
    pub t_right: char,
    /// T-junction pointing left (for right edge separators)
    pub t_left: char,
}

impl BoxChars {
    /// Standard rectangle box characters
    pub fn rectangle(style: CharacterSet) -> Self {
        match style {
            CharacterSet::Ascii | CharacterSet::Compact => Self::ascii(),
            _ => Self::unicode(),
        }
    }

    /// Rounded rectangle box characters
    pub fn rounded(style: CharacterSet) -> Self {
        match style {
            CharacterSet::Ascii | CharacterSet::Compact => Self::ascii(),
            _ => Self {
                top_left: '╭',
                top_right: '╮',
                bottom_left: '╰',
                bottom_right: '╯',
                horizontal: '─',
                vertical: '│',
                t_right: '├',
                t_left: '┤',
            },
        }
    }

    /// Double-line box for subgraphs/containers (visually distinct from nodes)
    pub fn double(style: CharacterSet) -> Self {
        match style {
            CharacterSet::Ascii | CharacterSet::Compact => Self {
                top_left: '#',
                top_right: '#',
                bottom_left: '#',
                bottom_right: '#',
                horizontal: '=',
                vertical: '#',
                t_right: '#',
                t_left: '#',
            },
            _ => Self {
                top_left: '╔',
                top_right: '╗',
                bottom_left: '╚',
                bottom_right: '╝',
                horizontal: '═',
                vertical: '║',
                t_right: '╠',
                t_left: '╣',
            },
        }
    }

    /// ASCII-only box characters
    pub fn ascii() -> Self {
        Self {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            t_right: '+',
            t_left: '+',
        }
    }

    /// Unicode box-drawing characters
    pub fn unicode() -> Self {
        Self {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
            t_right: '├',
            t_left: '┤',
        }
    }
}

impl Default for BoxChars {
    fn default() -> Self {
        Self::unicode()
    }
}

/// Line drawing characters for edges and connections
#[derive(Debug, Clone, Copy)]
pub struct LineChars {
    pub horizontal: char,
    pub vertical: char,
    pub arrow_up: char,
    pub arrow_down: char,
    pub arrow_left: char,
    pub arrow_right: char,
}

impl LineChars {
    /// Get line characters for the given style
    pub fn new(style: CharacterSet) -> Self {
        match style {
            CharacterSet::Ascii | CharacterSet::Compact => Self::ascii(),
            _ => Self::unicode(),
        }
    }

    /// ASCII line characters
    pub fn ascii() -> Self {
        Self {
            horizontal: '-',
            vertical: '|',
            arrow_up: '^',
            arrow_down: 'v',
            arrow_left: '<',
            arrow_right: '>',
        }
    }

    /// Unicode line characters
    pub fn unicode() -> Self {
        Self {
            horizontal: '─',
            vertical: '│',
            arrow_up: '▲',
            arrow_down: '▼',
            arrow_left: '◀',
            arrow_right: '▶',
        }
    }
}

impl Default for LineChars {
    fn default() -> Self {
        Self::unicode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_chars_ascii() {
        let chars = BoxChars::rectangle(CharacterSet::Ascii);
        assert_eq!(chars.top_left, '+');
        assert_eq!(chars.horizontal, '-');
    }

    #[test]
    fn test_box_chars_unicode() {
        let chars = BoxChars::rectangle(CharacterSet::Unicode);
        assert_eq!(chars.top_left, '┌');
        assert_eq!(chars.horizontal, '─');
    }

    #[test]
    fn test_box_chars_rounded() {
        let chars = BoxChars::rounded(CharacterSet::Unicode);
        assert_eq!(chars.top_left, '╭');
        assert_eq!(chars.bottom_right, '╯');
    }

    #[test]
    fn test_box_chars_double() {
        let chars = BoxChars::double(CharacterSet::Unicode);
        assert_eq!(chars.top_left, '╔');
        assert_eq!(chars.horizontal, '═');
    }

    #[test]
    fn test_line_chars_ascii() {
        let chars = LineChars::new(CharacterSet::Ascii);
        assert_eq!(chars.horizontal, '-');
        assert_eq!(chars.arrow_right, '>');
    }

    #[test]
    fn test_line_chars_unicode() {
        let chars = LineChars::new(CharacterSet::Unicode);
        assert_eq!(chars.horizontal, '─');
        assert_eq!(chars.arrow_right, '▶');
    }
}
