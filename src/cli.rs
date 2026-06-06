//! The `ahr` command-line interface: the clap-derived argument shape and
//! the [`dispatch`] entry point. `ahr` always reads its input from stdin and
//! writes text to stdout; the optional `--format` flag forces how that input
//! is interpreted (default: auto-detect).

use std::io::{Read, Write};

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};

use crate::render;

/// Render diagrams (and, later, other inputs) from stdin as terminal text.
#[derive(Parser, Debug, Clone, Copy)]
#[command(
    name = "ahr",
    about = "render diagrams and other stdin inputs as terminal text",
    version,
    after_help = "reads stdin, writes text to stdout — e.g. `ahr < diagram.mmd`"
)]
pub(crate) struct Cli {
    /// How to interpret the input. `auto` (default) sniffs the input: a
    /// leading Mermaid diagram keyword is rendered as a diagram, anything
    /// else as Markdown.
    #[arg(long, value_enum, default_value_t = Format::Auto)]
    format: Format,
}

/// The supported input formats. `Auto` sniffs the input and resolves to
/// one of the concrete formats (see [`looks_like_mermaid`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    /// Detect the format from the input.
    Auto,
    /// A Mermaid diagram (flowchart, sequence, class, gitgraph, ...).
    Mermaid,
    /// A Markdown document, rendered as styled terminal text.
    Markdown,
}

/// Distinctive (camelCase / compound) Mermaid keywords that are very unlikely
/// to start a line of ordinary prose, so a leading match alone is conclusive.
const DISTINCTIVE_KEYWORDS: &[&str] = &[
    "sequenceDiagram",
    "classDiagram",
    "stateDiagram",
    "stateDiagram-v2",
    "erDiagram",
    "gitGraph",
    "mindmap",
    "quadrantChart",
    "requirementDiagram",
];

/// Mermaid keywords that collide with common English words (`pie charts`,
/// `graph traversal`, ...). They only count as a diagram header when the line
/// is the keyword alone or a recognised sub-directive — never a prose
/// sentence. `graph`/`flowchart` are handled separately (they need a
/// direction).
const AMBIGUOUS_KEYWORDS: &[&str] = &["pie", "gantt", "journey", "timeline"];

/// Flowchart directions that must follow `graph` / `flowchart` for the line
/// to be a diagram header rather than prose (`graph TD`, `flowchart LR`).
const FLOWCHART_DIRECTIONS: &[&str] = &["TB", "TD", "BT", "RL", "LR"];

/// The first line that could be a Mermaid diagram header, skipping a leading
/// YAML frontmatter block (`--- … ---`) and any `%%` init directives /
/// comments that legally precede the diagram keyword.
fn mermaid_header_line(input: &str) -> Option<&str> {
    let mut lines = input.lines().map(str::trim).filter(|line| !line.is_empty());
    let mut line = lines.next()?;
    if line == "---" {
        // Consume to the closing fence, then take the next content line.
        for inner in lines.by_ref() {
            if inner == "---" {
                break;
            }
        }
        line = lines.next()?;
    }
    while line.starts_with("%%") {
        line = lines.next()?;
    }
    Some(line)
}

/// Whether `input` looks like a Mermaid diagram. Uses a precision-biased
/// heuristic: misclassifying prose as a diagram makes rendering *fail*
/// (figurehead errors on non-diagram text), whereas the reverse just renders
/// a diagram's source as Markdown — so we only claim Mermaid when the header
/// line is unmistakably a diagram declaration. `--format` overrides either way.
fn looks_like_mermaid(input: &str) -> bool {
    let Some(line) = mermaid_header_line(input) else {
        return false;
    };
    let mut tokens = line.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    let second = tokens.next();

    if DISTINCTIVE_KEYWORDS.contains(&first) {
        return true;
    }
    if first == "graph" || first == "flowchart" {
        return second
            .map(|dir| dir.trim_end_matches(';'))
            .is_some_and(|dir| FLOWCHART_DIRECTIONS.contains(&dir));
    }
    if AMBIGUOUS_KEYWORDS.contains(&first) {
        // Stand-alone (`pie`) or a header sub-directive (`pie title …`,
        // `pie showData`); a prose sentence has some other second word.
        return matches!(second, None | Some("title" | "showData"));
    }
    false
}

/// Read all of stdin, render it per `--format`, and write the result to
/// stdout.
///
/// # Errors
/// Empty input, an I/O failure reading stdin or writing stdout, or a
/// rendering error from the selected format.
pub(crate) fn dispatch(cli: Cli) -> Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| anyhow::anyhow!("failed to read stdin: {err}"))?;

    if input.trim().is_empty() {
        bail!("no input on stdin");
    }

    // Resolve `auto` to a concrete format up front so the render match has
    // no `Auto` arm to fall through.
    let format = if cli.format == Format::Auto {
        if looks_like_mermaid(&input) {
            Format::Mermaid
        } else {
            Format::Markdown
        }
    } else {
        cli.format
    };

    // Arms kept explicit (no `_ =>`) so a new variant is a compile error.
    let rendered = match format {
        Format::Mermaid => render::mermaid(&input)?,
        Format::Markdown => render::markdown(&input),
        Format::Auto => unreachable!("Auto resolved to a concrete format above"),
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
