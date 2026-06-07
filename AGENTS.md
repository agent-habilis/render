# AGENTS.md

Guidance for AI agents working in this repository.

## Architecture

`ah-r` renders an input file as terminal text, picking a renderer from the file
extension. The layout follows from that:

- **One module folder per rendered type.** Each supported type lives in its own
  `src/<type>/mod.rs` (`markdown`, `mermaid`, `table`, `deck`, `img`, …)
  exposing a single `pub(crate)` entry function. `src/cli/mod.rs` maps each
  extension to one of them.
- **A type's helpers live inside that type's module**, not in a loose file. The
  `src/` root holds only `main.rs`, `lib.rs`, `cli/`, and the type folders — do
  not drop a `src/foo.rs` helper file next to them.
- **Deck components reuse the top-level type modules.** A `<...>` block in
  `src/deck/components/` calls into `crate::mermaid`, `crate::img`, etc.
  rather than duplicating that type's rendering logic.

When adding a new rendered type, add a `src/<type>/` folder and wire its
extension into `src/cli/mod.rs`; do not introduce a parallel top-level file.

## Comments

This codebase keeps comments sparse. Follow these rules:

- **Default to none.** Assume no comment. Add one only if a competent reader
  would otherwise misread the code or miss a non-obvious gotcha. Narrating
  intent or design the names and values already convey — "so it blends with the
  theme", "give it breathing room", "the only fills are…" — is not a *why*; cut
  it. A good *why* names a consequence you can't see locally: a crate quirk, an
  ordering constraint, a bug it prevents.
- **Only explain *why*.** A comment must add what the code cannot say on its
  own — rationale, a trade-off, a non-obvious constraint, or a gotcha that would
  trip up the next reader. If a comment merely restates what the code plainly
  does, delete it; the code is the source of truth for *what*.
- **No file-header comments.** Do not open a file with a `//!` module-doc block
  (or any banner) describing what the file is. The module path and its contents
  already say that.
- **Drop derivable doc comments.** A `///` that just paraphrases a function's
  name and signature is noise — remove it. Keep a doc comment only when it
  carries a *why* the signature can't.

### Load-bearing comments — keep these

Some comments are not commentary; removing them changes behavior or breaks the
build:

- **clap `///` docs** on `Commands` / `Args` / `Task` variants and fields render
  as the CLI's `--help` text.
- **`# Errors` sections on `pub` functions** are required by clippy `pedantic`
  (`missing_errors_doc`). `pub(crate)` and private functions don't need them.
- **`#[expect(..., reason = "...")]`** attributes — the `reason` is mandated by
  the `allow_attributes` lints, not optional prose.
