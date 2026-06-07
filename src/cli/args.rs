//! The clap-derived argument shape for the `ahr` command.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use super::setup::Agent;

/// Render a diagram or document file as terminal text.
#[derive(Parser, Debug)]
#[command(
    name = "ahr",
    about = "render diagrams and documents as terminal text",
    version,
    arg_required_else_help = true,
    args_conflicts_with_subcommands = true,
    after_help = "the file extension selects the renderer — e.g. `ahr diagram.mmd`\n`.ah-r` opens a full-screen presentation (arrow keys navigate, q quits)\n\na tool by agent-habilis █🫈"
)]
pub(crate) struct Cli {
    /// Path to the input file. The extension selects the renderer:
    /// `.mmd`/`.mermaid` → Mermaid diagram, `.md`/`.markdown` → Markdown,
    /// `.ah-r` → full-screen presentation.
    pub(super) path: Option<PathBuf>,
    #[command(subcommand)]
    pub(super) command: Option<Command>,
}

/// Subcommands beyond the default file render.
#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Install the `/render` agent skill into your agents.
    ///
    /// Dry run by default (prints what it would install); `--execute` writes.
    /// With no `--agent` it installs into the agents detected on this machine.
    Setup {
        /// Actually write the skill. Without it this is a dry run. `--execute`
        /// replaces each target's `render/` directory (deletes whatever is
        /// there, then reinstalls).
        #[arg(long)]
        execute: bool,
        /// Agent(s) to install into (repeatable): `claude`, `pi`, or `generic`.
        #[arg(long = "agent", value_enum)]
        agents: Vec<Agent>,
    },

    /// Remove the `/render` agent skill from your agents (symmetric to `setup`).
    ///
    /// Dry run by default (prints what it would remove); `--execute` removes.
    /// With no `--agent` it targets the agents that have `/render` installed.
    Teardown {
        /// Actually remove the skill. Without it this is a dry run.
        #[arg(long)]
        execute: bool,
        /// Agent(s) to remove from (repeatable): `claude`, `pi`, or `generic`.
        #[arg(long = "agent", value_enum)]
        agents: Vec<Agent>,
    },
}
