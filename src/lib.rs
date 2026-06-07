//! `agent-habilis-render` — render an input file as terminal text.
//!
//! This crate ships as **both** a binary (the `ahr` CLI) and a library. The
//! binary is a thin shim over [`run_cli`]. It reads the input file given as
//! its argument and selects a renderer from the extension: Mermaid diagrams via
//! the [`figurehead`] crate (`.mmd`/`.mermaid`), Markdown via [`termimad`]
//! (`.md`/`.markdown`), CSV/TSV tables via [`comfy_table`] (`.csv`/`.tsv`), and
//! full-screen presentations (`.ah-r`) navigated with the arrow keys.

// The public surface is just `run_cli`; everything else stays `pub(crate)`.
pub(crate) mod cli;
pub(crate) mod deck;
pub(crate) mod markdown;
pub(crate) mod mermaid;
pub(crate) mod table;

pub use cli::run_cli;
