//! The Markdown type: a document styled for the terminal, with any embedded
//! ` ```mermaid ` blocks drawn inline.

use std::io::IsTerminal;
use std::ops::Range;

use crate::mermaid::mermaid;

/// Render a Markdown document as styled terminal text, drawing any embedded
/// ` ```mermaid ` fenced blocks inline. A block figurehead cannot draw falls
/// back to its original code-block rendering, so this never loses content.
pub(crate) fn markdown(input: &str) -> String {
    markdown_to_width(input, wrap_width())
}

/// Render Markdown as styled terminal text wrapped to `width`, **without** the
/// embedded-diagram handling of [`markdown`]. Decks use this so a ` ```mermaid `
/// fence is just a styled code block (diagrams come from the `<mermaid>`
/// component instead).
pub(crate) fn markdown_plain(input: &str, width: usize) -> String {
    termimad(input, width)
}

/// Like [`markdown`] but at an explicit wrap `width`, splicing each
/// ` ```mermaid ` block's drawn diagram in where its source was.
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
    let skin = termimad::MadSkin::default();
    skin.text(input, Some(width)).to_string()
}

/// Append newlines so `out` ends with a blank line (or is empty), giving a drawn
/// diagram breathing room from the prose above it.
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

/// Byte range of each whole ` ```mermaid ` block and the source between fences,
/// in source order. Tilde/indented fences and an unclosed fence are ignored.
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
