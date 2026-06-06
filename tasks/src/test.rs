use xshell::{Shell, cmd};

use crate::TaskOutcome;

pub(crate) fn run(sh: &Shell) -> TaskOutcome {
    // `--workspace` so the `tasks` member is tested too, not just the root
    // package (the workspace has a root package, so a bare `cargo test`
    // would skip the other members).
    cmd!(sh, "cargo test --workspace").quiet().run()?;
    Ok(())
}
