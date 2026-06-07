# render (`ah-r`)

A small Rust binary in the agent-habilis family. It **renders a file as
terminal text**, choosing the renderer from the file's extension:

- **[Mermaid](https://mermaid.js.org/) diagrams** (`.mmd`, `.mermaid`) as
  terminal diagrams (flowchart, sequence, class, gitgraph), via
  [`figurehead`](https://crates.io/crates/figurehead).
- **Markdown** (`.md`, `.markdown`) as styled terminal text (headings, lists,
  blockquotes, inline code, balanced tables, width-wrapped), via
  [`termimad`](https://crates.io/crates/termimad). Embedded ` ```mermaid `
  blocks are drawn inline as diagrams.
- **Tables** (`.csv`, `.tsv`) as a rounded-corner terminal grid — the first row
  is the header, every row and column is separated, and cells wrap to fit the
  terminal — via [`comfy-table`](https://crates.io/crates/comfy-table).
- **Decks** (`.ahr-deck`) as a full-screen terminal presentation — Markdown slides
  with embedded code, diagrams, live shells, animations, and images (drawn with
  the terminal's kitty/iTerm2/Sixel graphics protocol). `--watch` live-reloads
  the deck as you edit it. See [the deck format](docs/deck-format.md).
- **Images** (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`) drawn inline with the
  terminal's kitty/iTerm2/Sixel graphics protocol (Unicode halfblocks as a
  fallback), via [`ratatui-image`](https://crates.io/crates/ratatui-image). The
  image is printed at the cursor and the prompt returns below it; this needs an
  interactive terminal.

## Usage

```sh
ah-r diagram.mmd
ah-r notes.md
```

The extension picks the renderer; an unsupported extension is an error. Output
is written to stdout, so it pipes cleanly:

```sh
ah-r notes.md | less -R
```

### Examples

Sample inputs live in [`examples/`](examples/):

```sh
ah-r examples/flowchart-td.mmd
ah-r examples/sequence.mmd
ah-r examples/table.csv
ah-r examples/sample.png
```

## Build

```sh
cargo build --release
```

The binary is named `ah-r`.

## License

MIT
