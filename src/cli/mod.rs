mod args;
mod setup;

use std::io::{IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::{deck, imageview, markdown, mermaid, table};
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
            render_file(&path, cli.watch)
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
    Image,
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
        Some("png" | "jpg" | "jpeg" | "gif" | "webp") => Some(Format::Image),
        _ => None,
    }
}

fn render_file(path: &Path, watch: bool) -> Result<()> {
    // Resolve the renderer before reading, so a bad extension fails fast.
    let Some(format) = format_for_extension(path) else {
        bail!(
            "unsupported input file `{}`: expected a .mmd/.mermaid, .md/.markdown, .csv, .tsv, .ah-r, or image (.png/.jpg/.jpeg/.gif/.webp) extension",
            path.display()
        );
    };

    // An image is binary and takes over terminal I/O to draw itself, so it is
    // handled before the text read (`read_to_string` would choke on the bytes).
    // It needs an interactive TTY: stdout to draw, stdin for the cursor query
    // that anchors the inline viewport.
    if format == Format::Image {
        if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
            bail!(
                "`{}` is an image; run it in an interactive terminal (not a pipe or redirect)",
                path.display()
            );
        }
        return imageview::render(path);
    }

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
            return deck::run(&input, path, watch);
        }
        // Images are binary and return before the text read above.
        Format::Image => unreachable!("images are handled before read_to_string"),
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
        // A reader closing the pipe early (`ah-r … | head`) is a clean end for a
        // filter; Rust ignores SIGPIPE, so EPIPE surfaces here instead.
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(anyhow::anyhow!("failed to write stdout: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, format_for_extension};
    use std::path::Path;

    #[test]
    fn maps_image_extensions_case_insensitively() {
        for ext in ["png", "jpg", "jpeg", "gif", "webp", "PNG", "JPG", "WebP"] {
            let path = format!("pic.{ext}");
            assert_eq!(
                format_for_extension(Path::new(&path)),
                Some(Format::Image),
                "{path} should be an image",
            );
        }
    }

    #[test]
    fn keeps_existing_format_mapping() {
        assert_eq!(format_for_extension(Path::new("a.mmd")), Some(Format::Mermaid));
        assert_eq!(format_for_extension(Path::new("a.md")), Some(Format::Markdown));
        assert_eq!(format_for_extension(Path::new("a.csv")), Some(Format::Table(b',')));
        assert_eq!(format_for_extension(Path::new("a.ah-r")), Some(Format::Deck));
        assert_eq!(format_for_extension(Path::new("a.bmp")), None);
    }
}
