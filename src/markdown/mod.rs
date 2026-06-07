use std::io::IsTerminal;
use std::ops::Range;

use termimad::crossterm::style::{Attribute, Color};
use termimad::{Alignment, CompoundStyle, LineStyle, MadSkin, StyledChar, rgb};

mod highlight;

pub(crate) use highlight::render_code;

use crate::mermaid::mermaid;

// Tokyo Night Moon palette.
const FG: Color = rgb(0xc8, 0xd3, 0xf5);
const MUTED: Color = rgb(0x63, 0x6d, 0xa6);
const BLUE: Color = rgb(0x82, 0xaa, 0xff);
const CYAN: Color = rgb(0x86, 0xe1, 0xfc);
const GREEN: Color = rgb(0xc3, 0xe8, 0x8d);
const YELLOW: Color = rgb(0xff, 0xc7, 0x77);
const ORANGE: Color = rgb(0xff, 0x96, 0x6c);
const INLINE_BG: Color = rgb(0x2f, 0x33, 0x4d);
const BLOCK_BG: Color = rgb(0x1e, 0x20, 0x30);

/// Full render with fenced blocks spliced in (see [`markdown_to_width`]).
pub(crate) fn markdown(input: &str) -> String {
    markdown_to_width(input, wrap_width())
}

/// Plain termimad, no fence splicing — for deck prose (decks use `<mermaid>`/`<code>`).
pub(crate) fn markdown_plain(input: &str, width: usize) -> String {
    termimad(input, width)
}

/// Splices each fenced block in where its source sat: a `mermaid` block as a
/// drawn diagram (falling back to a code box when it can't be drawn), every other
/// fence as a code box via [`render_code`]. Prose between blocks goes to termimad.
fn markdown_to_width(input: &str, width: usize) -> String {
    let blocks = fenced_blocks(input);
    if blocks.is_empty() {
        return termimad(input, width);
    }

    let mut out = String::new();
    let mut cursor = 0;
    for (range, lang, body) in blocks {
        let (start, end) = (range.start, range.end);

        // Each prose slice renders on its own — a fence is always a block boundary.
        let before = trim_blank_lines(&input[cursor..start]);
        if !before.is_empty() {
            out.push_str(&termimad(before, width));
        }

        ensure_blank_line(&mut out);
        if lang.eq_ignore_ascii_case("mermaid") {
            // Undrawable: fall back to the raw source as a code box.
            let drawn = mermaid(body).map(|diagram| diagram.trim_end_matches('\n').to_owned());
            out.push_str(&drawn.unwrap_or_else(|_| render_code("", body)));
        } else {
            out.push_str(&render_code(lang, body));
        }
        out.push_str("\n\n");

        cursor = end;
    }

    let after = trim_blank_lines(&input[cursor..]);
    if !after.is_empty() {
        out.push_str(&termimad(after, width));
    }

    // Drop the trailing "\n\n" so a doc ending in a fence gains no extra blank line.
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Strip leading and trailing blank (whitespace-only) lines, keeping the
/// indentation of the surviving content — unlike `str::trim`, which would also
/// drop the first line's leading spaces (reflowing indented prose after a block).
fn trim_blank_lines(text: &str) -> &str {
    let (mut start, mut end, mut offset, mut seen) = (0, 0, 0, false);
    for line in text.split_inclusive('\n') {
        let line_end = offset + line.len();
        if line.trim().is_empty() {
            if !seen {
                start = line_end;
            }
        } else {
            seen = true;
            end = line_end;
        }
        offset = line_end;
    }
    if seen {
        text[start..end].trim_end_matches('\n')
    } else {
        ""
    }
}

fn termimad(input: &str, width: usize) -> String {
    skin().text(input, Some(width)).to_string()
}

/// termimad's default underlines every heading and renders `h2`–`h6` alike; this
/// re-skins each element onto the palette with a distinct hue/weight.
fn skin() -> MadSkin {
    let mut skin = MadSkin::default();

    skin.headers[0] = header(BLUE, &[Attribute::Bold, Attribute::Underlined]);
    skin.headers[1] = header(CYAN, &[Attribute::Bold]);
    skin.headers[2] = header(GREEN, &[Attribute::Bold]);
    skin.headers[3] = header(YELLOW, &[]);
    skin.headers[4] = header(ORANGE, &[Attribute::Italic]);
    skin.headers[5] = header(MUTED, &[Attribute::Italic]);

    skin.paragraph = LineStyle::new(CompoundStyle::with_fg(FG), Alignment::Left);
    skin.bold = compound(FG, Attribute::Bold);
    skin.italic = compound(FG, Attribute::Italic);
    skin.strikeout = compound(MUTED, Attribute::CrossedOut);
    skin.inline_code = CompoundStyle::with_fgbg(FG, INLINE_BG);
    skin.code_block = LineStyle::new(CompoundStyle::with_fgbg(FG, BLOCK_BG), Alignment::Left);

    skin.bullet = StyledChar::from_fg_char(BLUE, '•');
    skin.quote_mark = StyledChar::from_fg_char(MUTED, '▐');
    skin.horizontal_rule = StyledChar::from_fg_char(MUTED, '─');
    skin.table = CompoundStyle::with_fg(MUTED).into();

    skin
}

/// Built fresh (not from the default) so its blanket underline doesn't leak in.
fn header(fg: Color, attrs: &[Attribute]) -> LineStyle {
    let mut style = CompoundStyle::with_fg(fg);
    for &attr in attrs {
        style.add_attr(attr);
    }
    LineStyle::new(style, Alignment::Left)
}

fn compound(fg: Color, attr: Attribute) -> CompoundStyle {
    let mut style = CompoundStyle::with_fg(fg);
    style.add_attr(attr);
    style
}

fn ensure_blank_line(out: &mut String) {
    if out.is_empty() {
        return;
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.ends_with("\n\n") {
        out.push('\n');
    }
}

/// Each fenced block: its byte range, the language (`""` when untagged), and the
/// inner content. Tilde/indented fences and an unclosed fence are ignored.
fn fenced_blocks(input: &str) -> Vec<(Range<usize>, &str, &str)> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, usize, &str)> = None;
    let mut offset = 0;

    for line in input.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        let trimmed = line.trim();

        if let Some((block_start, inner_start, lang)) = open {
            if is_close_fence(trimmed) {
                blocks.push((block_start..offset, lang, &input[inner_start..line_start]));
                open = None;
            }
        } else if let Some(lang) = open_fence(trimmed) {
            open = Some((line_start, offset, lang));
        }
    }

    blocks
}

/// The language of an opening ` ``` ` fence — `Some("")` when untagged — or
/// `None` for a non-fence line or a 4+ backtick line. Only the first token
/// counts, e.g. `rust` in ` ```rust ignore `.
fn open_fence(trimmed: &str) -> Option<&str> {
    let lang = trimmed
        .strip_prefix("```")?
        .split_whitespace()
        .next()
        .unwrap_or("");
    (!lang.contains('`')).then_some(lang)
}

fn is_close_fence(trimmed: &str) -> bool {
    trimmed.len() >= 3 && trimmed.bytes().all(|byte| byte == b'`')
}

/// Wrap column for stdout: the terminal width when stdout is a TTY, else a
/// deterministic `80` so piped output is stable.
fn wrap_width() -> usize {
    if std::io::stdout().is_terminal() {
        let (cols, _rows) = termimad::terminal_size();
        if cols > 0 {
            return cols as usize;
        }
    }
    80
}

#[cfg(test)]
mod tests {
    use super::markdown;

    #[test]
    fn language_tagged_fence_is_syntax_highlighted() {
        let out = markdown("```rust\nfn main() {}\n```\n");
        assert!(
            out.contains("\u{1b}[38;2;"),
            "expected 24-bit color escapes"
        );
        assert!(out.contains("fn"), "source text must survive highlighting");
    }

    #[test]
    fn untagged_fence_renders_a_generic_box() {
        let out = markdown("```\nplain text\n```\n");
        assert!(out.contains("plain text"));
        assert!(
            out.contains("48;2;47;51;77"),
            "untagged fence → generic box"
        );
    }
}
