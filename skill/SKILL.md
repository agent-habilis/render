---
name: render
description: Render a diagram, document, table, slide deck, or image as terminal text via the `ah-r` CLI — Mermaid diagrams (`.mmd`/`.mermaid`), Markdown (`.md`/`.markdown`), CSV/TSV tables (`.csv`/`.tsv`), `.ahr-deck` presentations, and images (`.png`/`.jpg`/`.jpeg`/`.gif`/`.webp`). Use when asked to draw, preview, or display a diagram/markdown/table/deck/image in the terminal.
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

# A presentation (.ahr-deck) opens full-screen — only in an interactive terminal:
ah-r slides.ahr-deck

# An image (.png/.jpg/.jpeg/.gif/.webp) prints inline — only in an interactive terminal:
ah-r photo.png
```

Pick the form that matches the file:

- **Diagram** → `ah-r <file>.mmd` (or any Markdown with a ` ```mermaid ` block).
- **Document** → `ah-r <file>.md`; pipe through `less -R` to preserve color.
- **Table** → `ah-r <file>.csv` (or `.tsv` for tab-separated); the first row is
  styled as the header and wide cells wrap to the terminal.
- **Deck** → `ah-r <file>.ahr-deck` takes over the terminal (arrow keys navigate,
  `q` quits). It needs an interactive TTY; in a pipe or non-interactive shell it
  errors instead of rendering, so run it directly for the user.
- **Image** → `ah-r <file>.png` (`.jpg`/`.jpeg`/`.gif`/`.webp`) prints the image
  inline and returns. Like a deck it needs an interactive TTY and errors in a
  pipe, so run it directly for the user.

An unsupported or missing extension is an error — report it rather than
guessing the format.

## Report

Present the rendered output to the user **verbatim** — exactly as `ah-r` emitted
it, byte for byte, with no edits, reformatting, re-wrapping, summarizing, or
"cleaning up". The rendered text *is* the deliverable; altering it defeats the
rendering. For a deck, hand control back to the user to navigate.

**Do not narrate the output.** Emit only the rendered block — no commentary,
interpretation, summary, or explanation of what the diagram/table/document
"shows", before or after it. The user can read it themselves. For example, after
rendering a class diagram, do **not** add a line like "This is the Animal base
class with Dog and Cat inheriting from it (the ─◁ arrows point back to the
parent)."
