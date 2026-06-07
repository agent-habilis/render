# Render Markdown in the Terminal

`ah-r` reads Markdown on stdin and prints a **styled** terminal view using
[termimad](https://crates.io/crates/termimad).

## Text formatting

This paragraph mixes **bold**, *italic*, ***bold italic***, `inline code`,
and a [link](https://example.com). Long paragraphs wrap to the terminal
width (or 80 columns when the output is piped) so nothing runs off the edge.

## Lists

Unordered, with nesting:

- First item
- Second item
  - Nested item
  - Another nested item
- Third item

Ordered:

1. Clone the repo
2. Build the binary
3. Pipe a file in

## Blockquote

> Markdown is a lightweight markup language.
> termimad gives it color and structure in the terminal.

## Code block

```rust
fn main() {
    println!("hello, terminal markdown");
}
```

## Table

| Format   | Engine     | Output            |
| -------- | ---------- | ----------------- |
| mermaid  | figurehead | box-drawing chars |
| markdown | termimad   | styled ANSI text  |
