//! The `ahr` command-line interface: the clap-derived argument shape and
//! the [`dispatch`] entry point. `ahr` reads the file given as its positional
//! argument and writes text to stdout; the file's extension selects the renderer.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::render;

/// Render a diagram or document file as terminal text.
#[derive(Parser, Debug)]
#[command(
    name = "ahr",
    about = "render diagrams and documents as terminal text",
    version,
    after_help = "the file extension selects the renderer — e.g. `ahr diagram.mmd`"
)]
pub(crate) struct Cli {
    /// Path to the input file. The extension selects the renderer:
    /// `.mmd`/`.mermaid` → Mermaid diagram, `.md`/`.markdown` → Markdown.
    path: PathBuf,
}

/// Which renderer to drive, resolved from the input file's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// A Mermaid diagram (flowchart, sequence, class, gitgraph, ...).
    Mermaid,
    /// A Markdown document, rendered as styled terminal text.
    Markdown,
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
            "unsupported input file `{}`: expected a .mmd/.mermaid or .md/.markdown extension",
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
