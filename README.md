# render (`ahr`)

A small Rust binary in the agent-habilis family. It **reads an input on stdin
and writes text to stdout** — a Unix filter.

It renders two input formats, auto-detected from the input:

- **[Mermaid](https://mermaid.js.org/) diagrams** (flowchart, sequence, class,
  gitgraph) as terminal diagrams, via
  [`figurehead`](https://crates.io/crates/figurehead).
- **Markdown** as styled terminal text (headings, lists, blockquotes, inline
  code, balanced tables, width-wrapped), via
  [`termimad`](https://crates.io/crates/termimad).

## Usage

```sh
ahr < diagram.mmd
ahr < notes.md
# or
cat notes.md | ahr
```

`auto` (the default) treats input that starts with a Mermaid diagram keyword
(`graph`, `flowchart`, `sequenceDiagram`, ...) as a diagram, and everything
else as Markdown. Force it with `--format`:

```sh
ahr --format mermaid  < diagram.mmd
ahr --format markdown < notes.md
```

### Examples

Ready-to-pipe sample diagrams live in [`examples/`](examples/):

```sh
ahr < examples/flowchart-td.mmd
ahr < examples/sequence.mmd
```

Or inline:

```sh
printf 'graph TD\n  A[Start] --> B{Decision}\n  B -->|yes| C[Do thing]\n  B -->|no| D[Stop]\n' | ahr
```

## Build

```sh
cargo build --release
```

The binary is named `ahr`.

## License

MIT
