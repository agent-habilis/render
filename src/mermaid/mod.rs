use anyhow::{Context, Result};

pub(crate) fn mermaid(input: &str) -> Result<String> {
    figurehead::render(input).context("rendering mermaid diagram")
}
