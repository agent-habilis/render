//! The clap-derived argument shape for the `ahr` command.

use std::path::PathBuf;

use clap::Parser;

/// Render a diagram or document file as terminal text.
#[derive(Parser, Debug)]
#[command(
    name = "ahr",
    about = "render diagrams and documents as terminal text",
    version,
    arg_required_else_help = true,
    after_help = "the file extension selects the renderer — e.g. `ahr diagram.mmd`\n`.ah-r` opens a full-screen presentation (arrow keys navigate, q quits)\n\na tool by agent-habilis █🫈"
)]
pub(crate) struct Cli {
    /// Path to the input file. The extension selects the renderer:
    /// `.mmd`/`.mermaid` → Mermaid diagram, `.md`/`.markdown` → Markdown,
    /// `.ah-r` → full-screen presentation.
    pub(super) path: PathBuf,
}
