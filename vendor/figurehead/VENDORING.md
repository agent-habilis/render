# Vendored `figurehead`

A verbatim copy of [`figurehead` 0.4.3](https://github.com/Mootikins/figurehead-rs)
(MIT, © Matthew Krohn), pinned in via `[patch.crates-io]` in the workspace
`Cargo.toml`, carried for fixes upstream hasn't shipped. Each site is tagged
`LOCAL PATCH` in the source.

## The patches

`src/plugins/sequence/layout.rs` — the message-label width loop indexed
`widths[(left_idx + 1)..right_idx]` and divided by `right_idx - left_idx`
without handling a **self-message** (`A->>A:`), where `left_idx == right_idx`.
That is a reversed (out-of-bounds) slice and a divide-by-zero, so upstream
`figurehead::render` *panics* on any `sequenceDiagram` containing a self-message
(e.g. `swarm/p2p-binary-stream/docs/discovery.md`). The patch adds a
`continue` guard — a self-message spans no columns, so there is nothing to
widen.

`src/core/canvas.rs` — the grid stores one `char` per column and `Display`
emitted one char per cell, but a **wide char** (emoji like 🐝, CJK, ...) occupies
two terminal columns. Text drawn char-by-char therefore pushed everything to its
right by one column, breaking the vertical alignment of lifelines and box
borders on any row with an emoji. The patch makes `draw_text` / `draw_text_centered`
advance by display width (`unicode-width`) and reserve the trailing column with a
`WIDE_CONT` sentinel that `Display` skips, restoring a 1:1 grid↔terminal column
mapping. Layout/box sizing already used display width, so only the canvas needed
aligning.

## Re-syncing on upgrade

`Cargo.toml` here is slimmed to the library target and its runtime deps
(upstream's test/example targets and dev-dependencies are dropped, since the
patch only builds the lib). To move to a newer upstream: re-copy `src/`, keep
this manifest, and re-apply the `LOCAL PATCH` sites if upstream still lacks them.
