//! The Markdown type: a document styled for the terminal, with any embedded
//! ` ```mermaid ` blocks drawn inline.

use std::io::IsTerminal;
use std::ops::Range;

use termimad::crossterm::style::{Attribute, Color};
use termimad::{Alignment, CompoundStyle, LineStyle, MadSkin, rgb};

use crate::mermaid::mermaid;

/// Draws any embedded ` ```mermaid ` fenced blocks inline. A block figurehead
/// cannot draw falls back to its original code-block rendering, so this never
/// loses content.
pub(crate) fn markdown(input: &str) -> String {
    markdown_to_width(input, wrap_width())
}

/// **Without** the embedded-diagram handling of [`markdown`]: decks use this so
/// a ` ```mermaid ` fence is just a styled code block (diagrams come from the
/// `<mermaid>` component instead).
pub(crate) fn markdown_plain(input: &str, width: usize) -> String {
    termimad(input, width)
}

/// Splices each ` ```mermaid ` block's drawn diagram in where its source was.
fn markdown_to_width(input: &str, width: usize) -> String {
    let blocks = mermaid_blocks(input);
    if blocks.is_empty() {
        return termimad(input, width);
    }

    let mut out = String::new();
    let mut cursor = 0;
    for (range, src) in blocks {
        let (start, end) = (range.start, range.end);

        // termimad renders the markdown between blocks independently — safe
        // because a fenced block is always a block-level boundary.
        let before = &input[cursor..start];
        if !before.trim().is_empty() {
            out.push_str(&termimad(before, width));
        }

        match mermaid(src) {
            Ok(diagram) => {
                ensure_blank_line(&mut out);
                out.push_str(diagram.trim_end_matches('\n'));
                out.push_str("\n\n");
            }
            // Undrawable (unsupported type / bad syntax): keep the original
            // fence as a code block so nothing is lost.
            Err(_) => out.push_str(&termimad(&input[start..end], width)),
        }

        cursor = end;
    }

    let after = &input[cursor..];
    if !after.trim().is_empty() {
        out.push_str(&termimad(after, width));
    }

    out
}

fn termimad(input: &str, width: usize) -> String {
    skin().text(input, Some(width)).to_string()
}

/// termimad's default skin only bolds `h1`, underlines every level, and renders
/// `h2`–`h6` identically otherwise. This gives each level a distinct color and
/// weight so the heading hierarchy reads at a glance (palette from the Tokyo
/// Night code-highlight theme), left-aligned to match the surrounding prose.
fn skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin.headers[0] = header(rgb(0x82, 0xaa, 0xff), &[Attribute::Bold, Attribute::Underlined]);
    skin.headers[1] = header(rgb(0x86, 0xe1, 0xfc), &[Attribute::Bold]);
    skin.headers[2] = header(rgb(0xc3, 0xe8, 0x8d), &[Attribute::Bold]);
    skin.headers[3] = header(rgb(0xff, 0xc7, 0x77), &[]);
    skin.headers[4] = header(rgb(0xff, 0x96, 0x6c), &[Attribute::Italic]);
    skin.headers[5] = header(rgb(0x63, 0x6d, 0xa6), &[Attribute::Italic]);
    skin
}

/// One heading level: a left-aligned line in `fg` carrying `attrs` (and nothing
/// else — built fresh so the default's blanket underline does not leak in).
fn header(fg: Color, attrs: &[Attribute]) -> LineStyle {
    let mut style = CompoundStyle::with_fg(fg);
    for &attr in attrs {
        style.add_attr(attr);
    }
    LineStyle::new(style, Alignment::Left)
}

/// Gives a drawn diagram breathing room from the prose above it.
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

/// Tilde/indented fences and an unclosed fence are ignored.
fn mermaid_blocks(input: &str) -> Vec<(Range<usize>, &str)> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, usize)> = None;
    let mut offset = 0;

    for line in input.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        let trimmed = line.trim();

        if let Some((block_start, inner_start)) = open {
            if is_close_fence(trimmed) {
                blocks.push((block_start..offset, &input[inner_start..line_start]));
                open = None;
            }
        } else if is_mermaid_open_fence(trimmed) {
            open = Some((line_start, offset));
        }
    }

    blocks
}

fn is_mermaid_open_fence(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("```")
        .is_some_and(|info| info.trim().eq_ignore_ascii_case("mermaid"))
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
