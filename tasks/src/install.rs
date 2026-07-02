use std::path::{Path, PathBuf};

use xshell::{Shell, cmd};

use crate::TaskOutcome;
use crate::util::output;

pub(crate) fn run(sh: &Shell) -> TaskOutcome {
    output::status("Installing", "ahre");
    // Anchor `--path` at the repo root (parent of this `tasks/` crate),
    // computed at compile time, so the task installs `ahre` no matter which
    // directory it is invoked from — not whatever crate happens to be in the
    // caller's cwd.
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    // `--force` is required: the crate version rarely changes between builds,
    // and without `--force` `cargo install` treats "already installed" as
    // up-to-date and **skips the rebuild entirely**, silently leaving a stale
    // binary in place. `--force` always rebuilds + reinstalls the current tree.
    cmd!(sh, "cargo install --path {root} --force")
        .quiet()
        .run()?;
    output::status("Installed", "~/.cargo/bin/ahre");
    Ok(())
}
