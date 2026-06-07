# AGENTS.md

Guidance for AI agents working in this repository.

## Comments

This codebase keeps comments sparse. Follow these rules:

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
