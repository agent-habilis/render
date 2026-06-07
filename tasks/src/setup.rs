use std::path::{Path, PathBuf};

use xshell::{Shell, cmd};

use crate::TaskOutcome;
use crate::util::output;

/// Trailing args pass through to the binary's `setup` subcommand, so this is
/// dry-run unless `--execute` (the binary owns that behavior).
pub(crate) fn setup(sh: &Shell, args: &[String]) -> TaskOutcome {
    run_ah_r(sh, "setup", args)
}

/// `cargo task teardown [-- <args>]` — symmetric to [`setup`].
pub(crate) fn teardown(sh: &Shell, args: &[String]) -> TaskOutcome {
    run_ah_r(sh, "teardown", args)
}

/// Run `ah-r <subcommand> <args...>` via `cargo run`, anchored at the root crate
/// (parent of this `tasks/` crate, resolved at compile time) so it works from
/// any cwd — not whatever package happens to be in the caller's directory.
fn run_ah_r(sh: &Shell, subcommand: &str, args: &[String]) -> TaskOutcome {
    let manifest: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml");
    output::status("Running", &format!("ah-r {subcommand} {}", args.join(" ")));
    cmd!(sh, "cargo run --quiet --manifest-path {manifest} --bin ah-r -- {subcommand} {args...}")
        .run()?;
    Ok(())
}
