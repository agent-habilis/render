# examples

Sample inputs for `ahr`. Each is a standalone file you can pass to the binary:

```sh
ahr --path examples/flowchart-td.mmd
# during development:
cargo run --quiet -- --path examples/sequence.mmd
```

| File                | Kind / diagram type         |
| ------------------- | --------------------------- |
| `flowchart-td.mmd`  | Mermaid flowchart, top-down |
| `flowchart-lr.mmd`  | Mermaid flowchart, L-to-R   |
| `sequence.mmd`      | Mermaid sequence diagram    |
| `class.mmd`         | Mermaid class (braces form) |
| `gitgraph.mmd`      | Mermaid git graph           |
| `markdown.md`       | Markdown (styled via termimad) |
| `markdown-with-mermaid.md` | Markdown embedding a ` ```mermaid ` block |

> Note: figurehead parses class members in the brace-block form
> (`class Foo { +bar() }`), not the `Foo : +bar()` colon form.

These double as fixtures for the test pass to come.
