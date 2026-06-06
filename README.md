# render (`ahr`)

A small Rust binary in the agent-habilis family. It **renders a file as
terminal text**, choosing the renderer from the file's extension:

- **[Mermaid](https://mermaid.js.org/) diagrams** (`.mmd`, `.mermaid`) as
  terminal diagrams (flowchart, sequence, class, gitgraph), via
  [`figurehead`](https://crates.io/crates/figurehead).
- **Markdown** (`.md`, `.markdown`) as styled terminal text (headings, lists,
  blockquotes, inline code, balanced tables, width-wrapped), via
  [`termimad`](https://crates.io/crates/termimad). Embedded ` ```mermaid `
  blocks are drawn inline as diagrams.

## Usage

```sh
ahr --path diagram.mmd
ahr --path notes.md
```

The extension picks the renderer; an unsupported extension is an error. Output
is written to stdout, so it pipes cleanly:

```sh
ahr --path notes.md | less -R
```

### Examples

Sample inputs live in [`examples/`](examples/):

```sh
ahr --path examples/flowchart-td.mmd
ahr --path examples/sequence.mmd
```

## Build

```sh
cargo build --release
```

The binary is named `ahr`.

## License

MIT
