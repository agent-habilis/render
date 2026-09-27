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

`src/plugins/sequence/renderer.rs` — message arrows drew `─` straight over the
lifelines they touched (and dropped the arrowhead onto the receiver's `│`), so
every lifeline a message started at, crossed, or ended on was broken. The patch
keeps the lifelines intact with box-drawing junctions: `├`/`┤` where a message
leaves its sender, `┼` where it crosses an intermediate lifeline, and the
arrowhead now stops one column short of the receiver so its `│` survives
(`──▶│`). ASCII mode uses `+`.

`src/plugins/flowchart/layout.rs`, `src/plugins/flowchart/renderer.rs` — flowchart
edges:

- A **back edge between two members of one subgraph** was routed 2 cells past
  the whole diagram, while the subgraph frame was sized from its members only,
  so the edge left the frame and crossed it twice. The edge now routes 1 cell
  past the members, and that frame grows to hold the route. In LR/RL the
  canvas margin for back edges was added to the width instead of the height,
  so the route below the diagram fell off the canvas; it now goes to the
  height.
- A **TD/BT branch label** was drawn on the branch line, directly behind the
  arrowhead, so the arrowhead touched no line cell. The label now goes beside
  the line, on the side away from the junction, and stays on the left only
  when it fits above its own target box. `rank_sep` goes from 4 to 5, so a
  branch after a split junction is a corner, a line cell with the label beside
  it, a line cell, and the arrowhead. A single offset edge turns at the same
  row.
- **BT/RL arrowheads and back-edge arrowheads** stopped one cell short of the
  target box, because they entered one cell past its border (TD/LR enter at the
  border). They now enter at the border. The BT/RL split junction also sat
  against the source box; it now leaves one line cell, as TD/LR do.

## Re-syncing on upgrade

`Cargo.toml` here is slimmed to the library target and its runtime deps
(upstream's test/example targets and dev-dependencies are dropped, since the
patch only builds the lib). To move to a newer upstream: re-copy `src/`, keep
this manifest, and re-apply the `LOCAL PATCH` sites if upstream still lacks them.
