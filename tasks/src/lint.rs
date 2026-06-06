use xshell::{Shell, cmd};

use crate::TaskOutcome;

pub(crate) fn run(sh: &Shell) -> TaskOutcome {
    // `--workspace`: the runner lives in a member crate (`tasks`), but the
    // workspace has a root package, so a bare clippy would only check the
    // root and skip `tasks` itself. Lint every member.
    cmd!(sh, "cargo clippy --workspace --all-targets -- -D warnings")
        .quiet()
        .run()?;
    Ok(())
}
