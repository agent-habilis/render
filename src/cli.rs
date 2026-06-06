//! The `ahr` command-line interface: the clap-derived argument shape and
//! the [`dispatch`] entry point. `ahr` reads the file given as its positional
//! argument and writes text to stdout; the file's extension selects the renderer.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::{present, render};

/// Render a diagram or document file as terminal text.
#[derive(Parser, Debug)]
#[command(
    name = "ahr",
    about = "render diagrams and documents as terminal text",
    version,
    // No file argument (e.g. a bare `ahr`) prints the help screen instead of a
    // terse missing-argument error.
    arg_required_else_help = true,
    after_help = "the file extension selects the renderer — e.g. `ahr diagram.mmd`\n`.ah-r` opens a full-screen presentation (arrow keys navigate, q quits)\n\na tool by agent-habilis █🫈"
)]
pub(crate) struct Cli {
    /// Path to the input file. The extension selects the renderer:
    /// `.mmd`/`.mermaid` → Mermaid diagram, `.md`/`.markdown` → Markdown,
    /// `.ah-r` → full-screen presentation.
    path: PathBuf,
}

/// Which renderer to drive, resolved from the input file's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// A Mermaid diagram (flowchart, sequence, class, gitgraph, ...).
    Mermaid,
    /// A Markdown document, rendered as styled terminal text.
    Markdown,
    /// An `.ah-r` presentation, navigated full-screen.
    Deck,
}

/// Resolve the renderer from a file extension, case-insensitively. Returns
/// `None` for an unsupported or missing extension.
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

/// Read the input file (the positional argument), render it per its extension,
/// and write the result to stdout.
///
/// # Errors
/// An unsupported/missing extension, a file that can't be read (not found,
/// permission, non-UTF-8), empty contents, or a rendering error.
pub(crate) fn dispatch(cli: Cli) -> Result<()> {
    let Cli { path } = cli;

    // Resolve the renderer from the extension first, before reading.
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

    // Arms kept explicit (no `_ =>`) so a new variant is a compile error.
    let rendered = match format {
        Format::Mermaid => render::mermaid(&input)?,
        Format::Markdown => render::markdown(&input),
        // A presentation takes over the terminal instead of writing to stdout,
        // so it returns straight from here. It needs an interactive TTY on both
        // ends: stdout to draw, and stdin so key events reach the event loop
        // (otherwise it would strand on the alternate screen with no way to
        // quit).
        Format::Deck => {
            if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
                bail!(
                    "`{}` is a presentation; run it in an interactive terminal (not a pipe or redirect)",
                    path.display()
                );
            }
            return present::run(&input);
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
        // A downstream reader closing the pipe early (e.g. `ahr … | head`) is
        // a normal end for a filter, not a failure — exit cleanly. Rust
        // ignores SIGPIPE, so EPIPE surfaces here as an error instead.
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(anyhow::anyhow!("failed to write stdout: {err}")),
    }
}
