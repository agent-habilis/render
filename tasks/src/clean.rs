use xshell::{Shell, cmd};

use crate::TaskOutcome;

pub(crate) fn run(sh: &Shell) -> TaskOutcome {
    eprintln!("=> Cleaning build artifacts...");
    cmd!(sh, "cargo clean").quiet().run()?;
    Ok(())
}
