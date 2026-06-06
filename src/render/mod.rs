//! Input renderers. Each function turns a raw stdin string into the text
//! that `ahr` prints: [`mermaid`] draws diagrams, [`markdown`] styles
//! Markdown for the terminal.

use std::io::IsTerminal;

use anyhow::{Context, Result};

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

/// Render a Markdown document as styled terminal text.
///
/// Uses [`termimad`]'s default skin — styled headings, lists, blockquotes,
/// inline code, and balanced tables — wrapped to [`wrap_width`]. Infallible,
/// so unlike [`mermaid`] it returns the `String` directly.
pub(crate) fn markdown(input: &str) -> String {
    let skin = termimad::MadSkin::default();
    skin.text(input, Some(wrap_width())).to_string()
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
