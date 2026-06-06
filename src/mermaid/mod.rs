//! The Mermaid type: a diagram source drawn as terminal text.

use anyhow::{Context, Result};

/// Render a Mermaid diagram to terminal text via [`figurehead::render`].
///
/// # Errors
/// Propagates `figurehead`'s parse/render error when the input is not valid
/// Mermaid or a diagram type it cannot draw.
pub(crate) fn mermaid(input: &str) -> Result<String> {
    figurehead::render(input).context("rendering mermaid diagram")
}
