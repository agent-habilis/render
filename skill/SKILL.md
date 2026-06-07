---
name: render
description: Render a diagram, document, or slide deck as terminal text via the `ahr` CLI — Mermaid diagrams (`.mmd`/`.mermaid`), Markdown (`.md`/`.markdown`), and `.ah-r` presentations. Use when asked to draw, preview, or display a diagram/markdown/deck in the terminal.
allowed-tools: Bash(ahr:*)
---

# render

Turn a file into terminal output with the `ahr` CLI — the renderer is chosen
from the file extension. Use this when the user wants a diagram, Markdown
document, or slide deck shown in the terminal rather than described.

## Preflight

Confirm `ahr` is on `PATH`:

```sh
ahr --version
```

If it is missing, install it from the repo (`cargo task install`) or per the
README, then continue.

## Render a file

The extension selects the renderer; output goes to stdout, so it pipes cleanly.

```sh
# Mermaid diagram (.mmd / .mermaid) — flowchart, sequence, class, gitgraph:
ahr diagram.mmd

# Markdown (.md / .markdown) — styled text; embedded ```mermaid blocks draw inline:
ahr notes.md
ahr notes.md | less -R

# A presentation (.ah-r) opens full-screen — only in an interactive terminal:
ahr slides.ah-r
```

Pick the form that matches the file:

- **Diagram** → `ahr <file>.mmd` (or any Markdown with a ` ```mermaid ` block).
- **Document** → `ahr <file>.md`; pipe through `less -R` to preserve color.
- **Deck** → `ahr <file>.ah-r` takes over the terminal (arrow keys navigate,
  `q` quits). It needs an interactive TTY; in a pipe or non-interactive shell it
  errors instead of rendering, so run it directly for the user.

An unsupported or missing extension is an error — report it rather than
guessing the format.

## Report

Present the rendered output to the user **verbatim** — exactly as `ahr` emitted
it, byte for byte, with no edits, reformatting, re-wrapping, summarizing, or
"cleaning up". The rendered text *is* the deliverable; altering it defeats the
rendering. For a deck, hand control back to the user to navigate.
