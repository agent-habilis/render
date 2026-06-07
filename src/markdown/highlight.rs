use std::sync::LazyLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::as_24_bit_terminal_escaped;
use termimad::crossterm::style::{SetBackgroundColor, SetForegroundColor};

use super::{FG, INLINE_BG};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

static THEME: LazyLock<Theme> = LazyLock::new(|| {
    let bytes = include_bytes!("../../assets/themes/tokyonight_moon.tmTheme");
    ThemeSet::load_from_reader(&mut std::io::Cursor::new(&bytes[..]))
        .expect("vendored tokyonight_moon.tmTheme is a valid TextMate theme")
});

const RESET: &str = "\x1b[0m";

fn box_style() -> String {
    format!(
        "{}{}",
        SetForegroundColor(FG),
        SetBackgroundColor(INLINE_BG)
    )
}

pub(crate) fn render_code(lang: &str, code: &str) -> String {
    let syntaxes = &*SYNTAXES;
    let syntax = match syntaxes.find_syntax_by_token(normalize_lang(lang)) {
        // Plain Text carries no colors, so leave those for the plain box.
        Some(syntax) if syntax.name != "Plain Text" => Some(syntax),
        _ => None,
    };
    code_box(syntax, syntaxes, code)
}

fn code_box(syntax: Option<&SyntaxReference>, syntaxes: &SyntaxSet, code: &str) -> String {
    let lines: Vec<String> = trimmed_lines(code)
        .iter()
        .map(|&line| expand_tabs(line))
        .collect();
    let inner = lines
        .iter()
        .map(|line| console::measure_text_width(line))
        .max()
        .unwrap_or(0);
    let style = box_style();
    let padding = format!("{style}{}{RESET}", " ".repeat(inner + 2));
    let mut highlighter = syntax.map(|syntax| HighlightLines::new(syntax, &THEME));

    let mut out = String::new();
    out.push_str(&padding);
    for line in &lines {
        let content = highlight_line(highlighter.as_mut(), syntaxes, line);
        let fill = inner - console::measure_text_width(line) + 1; // +1: right margin
        out.push('\n');
        out.push_str(&style);
        out.push(' ');
        out.push_str(&content);
        out.push_str(&style); // re-assert the bg after the content's last fg
        out.push_str(&" ".repeat(fill));
        out.push_str(RESET);
    }
    out.push('\n');
    out.push_str(&padding);
    out
}

fn highlight_line(
    highlighter: Option<&mut HighlightLines<'_>>,
    syntaxes: &SyntaxSet,
    line: &str,
) -> String {
    let Some(highlighter) = highlighter else {
        return line.to_owned();
    };
    // syntect needs the trailing `\n` for correct multi-line state; strip it after.
    match highlighter.highlight_line(&format!("{line}\n"), syntaxes) {
        Ok(ranges) => as_24_bit_terminal_escaped(&ranges, false)
            .trim_end_matches('\n')
            .to_owned(),
        Err(_) => line.to_owned(),
    }
}

/// `measure_text_width` counts a tab as one cell but a terminal advances to a tab
/// stop, so expand tabs (every 4 columns) first to keep the box aligned.
fn expand_tabs(line: &str) -> String {
    const TAB: usize = 4;
    let mut out = String::new();
    let mut col = 0;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = TAB - (col % TAB);
            for _ in 0..spaces {
                out.push(' ');
            }
            col += spaces;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

fn trimmed_lines(code: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = code.lines().collect();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// Map a few language tokens that syntect's index does not recognise directly
/// onto ones it does. Everything else is passed through (syntect already matches
/// names and extensions case-insensitively, e.g. `rust`/`rs`, `js`, `yaml`).
fn normalize_lang(lang: &str) -> &str {
    match lang.trim().to_ascii_lowercase().as_str() {
        "shell" | "zsh" => "sh",
        "golang" => "go",
        "c++" => "cpp",
        "js" => "javascript",
        "ts" => "typescript",
        _ => lang.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::render_code;

    #[test]
    fn known_language_is_a_highlighted_box() {
        let out = render_code("rust", "fn main() {}\n");
        assert!(out.contains("\x1b[38;2;"), "expected 24-bit color escapes");
        assert!(out.contains("fn"), "source text must survive highlighting");
        assert!(
            out.contains("48;2;47;51;77"),
            "highlighted code sits in the box too"
        );
        assert!(out.ends_with("\x1b[0m"), "must end with a reset");
    }

    #[test]
    fn unknown_language_renders_a_generic_box() {
        let out = render_code("definitely-not-a-language", "just some text\n");
        assert!(out.contains("just some text"), "text must survive");
        assert!(
            out.contains("48;2;47;51;77"),
            "generic box uses the brighter bg fill"
        );
    }

    #[test]
    fn untagged_code_renders_a_generic_box() {
        let out = render_code("", "plain\n");
        assert!(out.contains("plain"));
        assert!(out.contains("48;2;47;51;77"), "no language → generic box");
    }
}
