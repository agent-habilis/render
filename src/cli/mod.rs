//! The `ahr` command-line interface: the argument shape ([`args`]) and the
//! [`run_cli`] entry point. `ahr` reads the file given as its positional
//! argument and writes text to stdout; the file's extension selects the renderer.

mod args;

use std::io::{IsTerminal, Write};

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::{deck, markdown, mermaid};
use args::Cli;

/// Parse `argv`, read the input file argument, render it, and write to stdout.
///
/// # Errors
/// An unsupported/missing extension, a file that can't be read (not found,
/// permission, non-UTF-8), empty contents, or a rendering error.
pub fn run_cli() -> Result<()> {
    dispatch(Cli::parse())
}

/// Which renderer to drive, resolved from the input file's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Mermaid,
    Markdown,
    Deck,
}

fn format_for_extension(path: &std::path::Path) -> Option<Format> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mmd" | "mermaid") => Some(Format::Mermaid),
        Some("md" | "markdown") => Some(Format::Markdown),
        Some("ah-r") => Some(Format::Deck),
        _ => None,
    }
}

fn dispatch(cli: Cli) -> Result<()> {
    let Cli { path } = cli;

    // Resolve the renderer before reading, so a bad extension fails fast.
    let Some(format) = format_for_extension(&path) else {
        bail!(
            "unsupported input file `{}`: expected a .mmd/.mermaid, .md/.markdown, or .ah-r extension",
            path.display()
        );
    };

    let input = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read `{}`", path.display()))?;

    if input.trim().is_empty() {
        bail!("`{}` is empty", path.display());
    }

    // Explicit arms (no `_ =>`) so a new variant is a compile error.
    let rendered = match format {
        Format::Mermaid => mermaid::mermaid(&input)?,
        Format::Markdown => markdown::markdown(&input),
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
