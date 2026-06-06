//! Input renderers. Each function turns an input string into the text that
//! `ahr` prints: [`mermaid`] draws diagrams, [`markdown`] styles Markdown for
//! the terminal (drawing any embedded ` ```mermaid ` blocks inline), and
//! [`highlight_code`] syntax-highlights a `<code>` block.

use std::io::IsTerminal;
use std::ops::Range;

use anyhow::{Context, Result};

mod highlight;

pub(crate) use highlight::highlight_code;

/// Render a Mermaid diagram to terminal text.
///
/// Delegates to [`figurehead::render`], which auto-detects the Mermaid
/// diagram type (flowchart, sequence, class, gitgraph, ...) and draws it
/// with Unicode box-drawing characters.
///
/// # Errors
/// Propagates `figurehead`'s parse/render error when the input is not valid
/// Mermaid or a diagram type it cannot draw.
pub(crate) fn mermaid(input: &str) -> Result<String> {
    figurehead::render(input).context("rendering mermaid diagram")
}

/// Render a Markdown document as styled terminal text, drawing any embedded
/// ` ```mermaid ` fenced blocks inline.
///
/// The markdown around each Mermaid block is styled with [`termimad`]; each
/// ` ```mermaid ` block is drawn with [`mermaid`] and spliced in where its
/// source used to be. A block that figurehead cannot draw (e.g. an
/// unsupported diagram type) falls back to its original code-block rendering,
/// so the function never loses content and stays infallible.
pub(crate) fn markdown(input: &str) -> String {
    markdown_to_width(input, wrap_width())
}

/// Render a Markdown document as styled terminal text wrapped to an explicit
/// `width`, drawing any embedded ` ```mermaid ` blocks inline.
///
/// Same behaviour as [`markdown`], but the caller chooses the wrap column
/// instead of inferring it from stdout. The presentation mode uses this to fit
/// a slide's content into a centered column narrower than the terminal.
pub(crate) fn markdown_to_width(input: &str, width: usize) -> String {
    let blocks = mermaid_blocks(input);
    if blocks.is_empty() {
        // Fast path: no embedded diagrams, render the whole document at once.
        return termimad(input, width);
    }

    let mut out = String::new();
    let mut cursor = 0;
    for (range, src) in blocks {
        let (start, end) = (range.start, range.end);

        // The markdown between the previous block and this one. termimad
        // renders it independently — safe because a fenced block is always a
        // block-level boundary.
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
            // Can't draw it (unsupported type / bad syntax): keep the original
            // fenced block, rendered as a code block, so nothing is lost.
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

/// Render Markdown as styled terminal text wrapped to `width`, **without** the
/// embedded-diagram handling of [`markdown_to_width`]. Decks use this so a
/// ` ```mermaid ` fence is just a styled code block (diagrams come from the
/// `<mermaid>` element instead).
pub(crate) fn markdown_plain(input: &str, width: usize) -> String {
    termimad(input, width)
}

/// Render markdown with [`termimad`]'s default skin — styled headings, lists,
/// blockquotes, inline code, and balanced tables — wrapped to `width` columns.
fn termimad(input: &str, width: usize) -> String {
    let skin = termimad::MadSkin::default();
    skin.text(input, Some(width)).to_string()
}

/// Append newlines so `out` ends with a blank line (or is empty), giving a
/// drawn diagram breathing room from the prose above it.
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

/// Find fenced ` ```mermaid ` blocks. Returns, in source order, the byte range
/// of each whole block (both fence lines included) and the Mermaid source
/// between the fences.
///
/// Hand-rolled line scanner: it recognises a ` ```mermaid ` open fence and the
/// next all-back-tick close fence. Tilde fences, indented fences, and
/// quad-back-tick nesting are out of scope; an unclosed open fence is ignored
/// (left to render as ordinary markdown).
fn mermaid_blocks(input: &str) -> Vec<(Range<usize>, &str)> {
    let mut blocks = Vec::new();
    // `(block_start, inner_start)` once an open fence has been seen.
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

/// A ` ```mermaid ` opening fence: a `` ``` `` prefix whose info string is
/// `mermaid` (case-insensitive).
fn is_mermaid_open_fence(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("```")
        .is_some_and(|info| info.trim().eq_ignore_ascii_case("mermaid"))
}

/// A closing fence: a line of three or more back-ticks and nothing else.
fn is_close_fence(trimmed: &str) -> bool {
    trimmed.len() >= 3 && trimmed.bytes().all(|byte| byte == b'`')
}

/// The column width to wrap rendered output to: the terminal's width when
/// stdout is a TTY, else a deterministic `80` so piped/redirected output is
/// stable.
fn wrap_width() -> usize {
    if std::io::stdout().is_terminal() {
        let (cols, _rows) = termimad::terminal_size();
        if cols > 0 {
            return cols as usize;
        }
    }
    80
}
