//! The `ahr` command-line interface: the argument shape ([`args`]) and the
//! [`run_cli`] entry point. With no subcommand `ahr` reads the file given as its
//! positional argument and writes the rendered text to stdout (the extension
//! selects the renderer); the `setup`/`teardown` subcommands manage the embedded
//! `/render` agent skill (see [`setup`]).

mod args;
mod setup;

use std::io::{IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::{deck, markdown, mermaid, table};
use args::{Cli, Command};

/// Render a file (default), or install/remove the `/render` skill.
///
/// # Errors
/// A bad extension, an unreadable or empty file, a rendering failure, or a
/// filesystem error while installing/removing the skill.
pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Setup { execute, agents }) => setup::setup(execute, &agents),
        Some(Command::Teardown { execute, agents }) => setup::teardown(execute, &agents),
        // `arg_required_else_help` guarantees a path when there is no subcommand.
        None => {
            let path = cli
                .path
                .ok_or_else(|| anyhow::anyhow!("no input file given"))?;
            render_file(&path)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Mermaid,
    Markdown,
    /// A delimited-data table; the byte is the field separator (`,` / tab).
    Table(u8),
    Deck,
}

fn format_for_extension(path: &Path) -> Option<Format> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mmd" | "mermaid") => Some(Format::Mermaid),
        Some("md" | "markdown") => Some(Format::Markdown),
        Some("csv") => Some(Format::Table(b',')),
        Some("tsv" | "tab") => Some(Format::Table(b'\t')),
        Some("ah-r") => Some(Format::Deck),
        _ => None,
    }
}

fn render_file(path: &Path) -> Result<()> {
    // Resolve the renderer before reading, so a bad extension fails fast.
    let Some(format) = format_for_extension(path) else {
        bail!(
            "unsupported input file `{}`: expected a .mmd/.mermaid, .md/.markdown, .csv, .tsv, or .ah-r extension",
            path.display()
        );
    };

    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read `{}`", path.display()))?;

    if input.trim().is_empty() {
        bail!("`{}` is empty", path.display());
    }

    // Explicit arms (no `_ =>`) so a new variant is a compile error.
    let rendered = match format {
        Format::Mermaid => mermaid::mermaid(&input)?,
        Format::Markdown => markdown::markdown(&input),
        Format::Table(delimiter) => table::table(&input, delimiter),
        // A presentation takes over the terminal and returns from here. It needs
        // an interactive TTY on both ends: stdout to draw, stdin so key events
        // reach the loop — otherwise it strands on the alternate screen.
        Format::Deck => {
            if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
                bail!(
                    "`{}` is a presentation; run it in an interactive terminal (not a pipe or redirect)",
                    path.display()
                );
            }
            return deck::run(&input);
        }
    };

    let mut stdout = std::io::stdout();
    let write = stdout
        .write_all(rendered.as_bytes())
        .and_then(|()| {
            if rendered.ends_with('\n') {
                Ok(())
            } else {
                stdout.write_all(b"\n")
            }
        })
        .and_then(|()| stdout.flush());

    match write {
        Ok(()) => Ok(()),
        // A reader closing the pipe early (`ahr … | head`) is a clean end for a
        // filter; Rust ignores SIGPIPE, so EPIPE surfaces here instead.
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(anyhow::anyhow!("failed to write stdout: {err}")),
    }
}
