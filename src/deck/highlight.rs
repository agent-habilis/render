use std::sync::LazyLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{LinesWithEndings, as_24_bit_terminal_escaped};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

static THEME: LazyLock<Theme> = LazyLock::new(|| {
    let bytes = include_bytes!("../../assets/themes/tokyonight_moon.tmTheme");
    ThemeSet::load_from_reader(&mut std::io::Cursor::new(&bytes[..]))
        .expect("vendored tokyonight_moon.tmTheme is a valid TextMate theme")
});

/// An unknown `lang` token falls back to plain text, so the code is still
/// returned verbatim — just uncolored.
pub(crate) fn highlight_code(lang: &str, code: &str) -> String {
    let syntaxes = &*SYNTAXES;
    let syntax = syntaxes
        .find_syntax_by_token(normalize_lang(lang))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, &THEME);
    let mut out = String::with_capacity(code.len() * 2);
    for line in LinesWithEndings::from(code) {
        // `highlight_line` only errors on a malformed syntax definition; ours
        // are vendored, so on the off chance it fails, fall back to the raw line.
        match highlighter.highlight_line(line, syntaxes) {
            Ok(ranges) => out.push_str(&as_24_bit_terminal_escaped(&ranges, false)),
            Err(_) => out.push_str(line),
        }
    }
    // Reset so trailing color never bleeds into whatever is drawn next.
    out.push_str("\x1b[0m");
    out
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
    use super::highlight_code;

    #[test]
    fn known_language_emits_truecolor_and_keeps_text() {
        let out = highlight_code("rust", "fn main() {}\n");
        assert!(out.contains("\x1b[38;2;"), "expected 24-bit color escapes");
        assert!(out.contains("fn"), "source text must survive highlighting");
        assert!(out.ends_with("\x1b[0m"), "must end with a reset");
    }

    #[test]
    fn unknown_language_falls_back_to_plain_text() {
        let out = highlight_code("definitely-not-a-language", "just some text\n");
        assert!(out.contains("just some text"));
    }
}
