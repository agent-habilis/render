---
name: render
description: Render a diagram, document, table, or slide deck as terminal text via the `ah-r` CLI — Mermaid diagrams (`.mmd`/`.mermaid`), Markdown (`.md`/`.markdown`), CSV/TSV tables (`.csv`/`.tsv`), and `.ah-r` presentations. Use when asked to draw, preview, or display a diagram/markdown/table/deck in the terminal.
allowed-tools: Bash(ah-r:*)
---

# render

Turn a file into terminal output with the `ah-r` CLI — the renderer is chosen
from the file extension. Use this when the user wants a diagram, Markdown
document, table, or slide deck shown in the terminal rather than described.

## Preflight

Confirm `ah-r` is on `PATH`:

```sh
ah-r --version
```

If it is missing, install it from the repo (`cargo task install`) or per the
README, then continue.

## Render a file

The extension selects the renderer; output goes to stdout, so it pipes cleanly.

```sh
# Mermaid diagram (.mmd / .mermaid) — flowchart, sequence, class, gitgraph:
ah-r diagram.mmd

# Markdown (.md / .markdown) — styled text; embedded ```mermaid blocks draw inline:
ah-r notes.md
ah-r notes.md | less -R

# Table (.csv / .tsv) — a rounded-corner grid; the first row is the header:
ah-r data.csv

# A presentation (.ah-r) opens full-screen — only in an interactive terminal:
ah-r slides.ah-r
```

Pick the form that matches the file:

- **Diagram** → `ah-r <file>.mmd` (or any Markdown with a ` ```mermaid ` block).
- **Document** → `ah-r <file>.md`; pipe through `less -R` to preserve color.
- **Table** → `ah-r <file>.csv` (or `.tsv` for tab-separated); the first row is
  styled as the header and wide cells wrap to the terminal.
- **Deck** → `ah-r <file>.ah-r` takes over the terminal (arrow keys navigate,
  `q` quits). It needs an interactive TTY; in a pipe or non-interactive shell it
  errors instead of rendering, so run it directly for the user.

An unsupported or missing extension is an error — report it rather than
guessing the format.

## Report

Present the rendered output to the user **verbatim** — exactly as `ah-r` emitted
it, byte for byte, with no edits, reformatting, re-wrapping, summarizing, or
"cleaning up". The rendered text *is* the deliverable; altering it defeats the
rendering. For a deck, hand control back to the user to navigate.
