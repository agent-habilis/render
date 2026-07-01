# Vendored `figurehead`

A verbatim copy of [`figurehead` 0.4.3](https://github.com/Mootikins/figurehead-rs)
(MIT, © Matthew Krohn), pinned in via `[patch.crates-io]` in the workspace
`Cargo.toml`, carried for a single fix upstream hasn't shipped.

## The patch

`src/plugins/sequence/layout.rs` — the message-label width loop indexed
`widths[(left_idx + 1)..right_idx]` and divided by `right_idx - left_idx`
without handling a **self-message** (`A->>A:`), where `left_idx == right_idx`.
That is a reversed (out-of-bounds) slice and a divide-by-zero, so upstream
`figurehead::render` *panics* on any `sequenceDiagram` containing a self-message
(e.g. `swarm/p2p-binary-stream/docs/discovery.md`). The patch adds a
`continue` guard — a self-message spans no columns, so there is nothing to
widen. Search the file for `LOCAL PATCH` for the exact site.

## Re-syncing on upgrade

`Cargo.toml` here is slimmed to the library target and its runtime deps
(upstream's test/example targets and dev-dependencies are dropped, since the
patch only builds the lib). To move to a newer upstream: re-copy `src/`, keep
this manifest, and re-apply the `LOCAL PATCH` guard if upstream still lacks it.
