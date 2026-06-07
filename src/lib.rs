// The public surface is just `run_cli`; everything else stays `pub(crate)`.
pub(crate) mod cli;
pub(crate) mod deck;
pub(crate) mod img;
pub(crate) mod imageview;
pub(crate) mod markdown;
pub(crate) mod mermaid;
pub(crate) mod table;

pub use cli::run_cli;
