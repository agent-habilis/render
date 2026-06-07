//! The Mermaid type: a diagram source drawn as terminal text.

use anyhow::{Context, Result};

/// Render a Mermaid diagram to terminal text via [`figurehead::render`].
pub(crate) fn mermaid(input: &str) -> Result<String> {
    figurehead::render(input).context("rendering mermaid diagram")
}
