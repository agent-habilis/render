//! Shared text utilities for diagram processing
//!
//! This module contains common text manipulation functions used across plugins.

use unicode_width::UnicodeWidthStr;

/// Wrap text to fit within a maximum width, breaking on word boundaries.
///
/// Returns a vector of lines, each fitting within `max_width` display columns.
/// If `max_width` is 0, or the label fits on one line, returns a single-element vector.
///
/// # Example
/// ```
/// use figurehead::core::wrap_label;
///
/// let lines = wrap_label("This is a long label", 10);
/// assert_eq!(lines, vec!["This is a", "long label"]);
/// ```
pub fn wrap_label(label: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || UnicodeWidthStr::width(label) <= max_width {
        return vec![label.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in label.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);

        if current_width == 0 {
            // First word on line
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + 1 + word_width <= max_width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Start new line
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_short_label() {
        let result = wrap_label("Hello", 20);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn test_wrap_exact_fit() {
        let result = wrap_label("Hello", 5);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn test_wrap_long_label() {
        let result = wrap_label("This is a long label", 10);
        assert_eq!(result, vec!["This is a", "long label"]);
    }

    #[test]
    fn test_wrap_zero_width() {
        let result = wrap_label("Hello World", 0);
        assert_eq!(result, vec!["Hello World"]);
    }

    #[test]
    fn test_wrap_empty_label() {
        let result = wrap_label("", 10);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn test_wrap_unicode() {
        // Japanese characters are typically 2 columns wide
        // wrap_label splits on whitespace, so words without spaces don't wrap
        let result = wrap_label("日本 語テスト", 6);
        // With max_width=6, "日本" (4 cols) fits, "語テスト" (8 cols) on next line
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "日本");
        assert_eq!(result[1], "語テスト");
    }

    #[test]
    fn test_wrap_multiple_lines() {
        let result = wrap_label("one two three four five", 8);
        assert_eq!(result, vec!["one two", "three", "four", "five"]);
    }
}
