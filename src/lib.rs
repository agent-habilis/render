//! `agent-habilis-render` — render an input file as terminal text.
//!
//! This crate ships as **both** a binary (the `ahr` CLI) and a library. The
//! binary is a thin shim over [`run_cli`]. It reads the input file given as
//! its argument and selects a renderer from the extension: Mermaid diagrams via
//! the [`figurehead`] crate (`.mmd`/`.mermaid`) and Markdown via
//! [`termimad`] (`.md`/`.markdown`).

// Internal modules stay `pub(crate)`: the curated public surface is just
// `run_cli`, called by the `src/main.rs` shim.
pub(crate) mod cli;
pub(crate) mod render;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

/// Parse `argv`, read the input file argument, render it, and write the result
/// to stdout.
///
/// This is the entire body of the `ahr` binary; it is public so the thin
/// `src/main.rs` shim can call it.
///
/// # Errors
/// Propagates any error from resolving/reading the input file or rendering it
/// — unsupported extension, unreadable file, empty input, or malformed syntax.
pub fn run_cli() -> Result<()> {
    cli::dispatch(Cli::parse())
}
