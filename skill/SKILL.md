---
name: render
description: Render a diagram, document, table, slide deck, or image as terminal text via the `ahre` CLI — Mermaid diagrams (`.mmd`/`.mermaid`), Markdown (`.md`/`.markdown`), CSV/TSV tables (`.csv`/`.tsv`), `.ahr-deck` presentations, and images (`.png`/`.jpg`/`.jpeg`/`.gif`/`.webp`). Use when asked to draw, preview, or display a diagram/markdown/table/deck/image in the terminal.
allowed-tools: Bash(ahre:*)
---

# render

Turn a file into terminal output with the `ahre` CLI — the renderer is chosen
from the file extension. Use this when the user wants a diagram, Markdown
document, table, or slide deck shown in the terminal rather than described.

## Preflight

Confirm `ahre` is on `PATH`:

```sh
ahre --version
```

If it is missing, install it from the repo (`cargo task install`) or per the
README, then continue.

## Render a file

The extension selects the renderer; output goes to stdout, so it pipes cleanly.

```sh
# Mermaid diagram (.mmd / .mermaid) — flowchart, sequence, class, gitgraph:
ahre diagram.mmd

# Markdown (.md / .markdown) — styled text; embedded ```mermaid blocks draw inline:
ahre notes.md
ahre notes.md | less -R

# Table (.csv / .tsv) — a rounded-corner grid; the first row is the header:
ahre data.csv

# A presentation (.ahr-deck) opens full-screen — only in an interactive terminal:
ahre slides.ahr-deck

# An image (.png/.jpg/.jpeg/.gif/.webp) prints inline — only in an interactive terminal:
ahre photo.png
```

Pick the form that matches the file:

- **Diagram** → `ahre <file>.mmd` (or any Markdown with a ` ```mermaid ` block).
- **Document** → `ahre <file>.md`; pipe through `less -R` to preserve color.
- **Table** → `ahre <file>.csv` (or `.tsv` for tab-separated); the first row is
  styled as the header and wide cells wrap to the terminal.
- **Deck** → `ahre <file>.ahr-deck` takes over the terminal (arrow keys navigate,
  `q` quits). It needs an interactive TTY; in a pipe or non-interactive shell it
  errors instead of rendering, so run it directly for the user.
- **Image** → `ahre <file>.png` (`.jpg`/`.jpeg`/`.gif`/`.webp`) prints the image
  inline and returns. Like a deck it needs an interactive TTY and errors in a
  pipe, so run it directly for the user.

An unsupported or missing extension is an error — report it rather than
guessing the format.

## Report

Present the rendered output to the user **verbatim** — exactly as `ahre` emitted
it, byte for byte, with no edits, reformatting, re-wrapping, summarizing, or
"cleaning up". The rendered text *is* the deliverable; altering it defeats the
rendering. For a deck, hand control back to the user to navigate.

**Do not narrate the output.** Emit only the rendered block — no commentary,
interpretation, summary, or explanation of what the diagram/table/document
"shows", before or after it. The user can read it themselves. For example, after
rendering a class diagram, do **not** add a line like "This is the Animal base
class with Dog and Cat inheriting from it (the ─◁ arrows point back to the
parent)."
