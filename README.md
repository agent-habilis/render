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
- **Decks** (`.ah-r`) as a full-screen terminal presentation — Markdown slides
  with embedded code, diagrams, live shells, and animations. See
  [the deck format](docs/deck-format.md).

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
```

## Build

```sh
cargo build --release
```

The binary is named `ah-r`.

## License

MIT
