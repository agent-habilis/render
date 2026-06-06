//! `agent-habilis-render` — read an input on stdin, write text to stdout.
//!
//! This crate ships as **both** a binary (the `ahr` CLI) and a library. The
//! binary is a thin shim over [`run_cli`]. The first and only input format
//! today is Mermaid, rendered to a terminal diagram via the [`figurehead`]
//! crate; the CLI is shaped so additional input formats slot in behind the
//! same `stdin -> text` contract.

// Internal modules stay `pub(crate)`: the curated public surface is just
// `run_cli`, called by the `src/main.rs` shim.
pub(crate) mod cli;
pub(crate) mod render;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

/// Parse `argv`, read stdin, render, and write the result to stdout.
///
/// This is the entire body of the `ahr` binary; it is public so the thin
/// `src/main.rs` shim can call it.
///
/// # Errors
/// Propagates any error from reading stdin or rendering the input — empty
/// input, malformed diagram syntax, or an I/O failure on stdin.
pub fn run_cli() -> Result<()> {
    cli::dispatch(Cli::parse())
}
