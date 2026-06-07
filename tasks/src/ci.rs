use xshell::{Shell, cmd};

use crate::TaskOutcome;
use crate::util::output;

pub(crate) fn run(sh: &Shell) -> TaskOutcome {
    // `fmt --check` can't share `fmt::run` (that one rewrites files); the
    // clippy and test steps reuse `lint`/`test` so the canonical commands
    // live in exactly one place.
    output::status("Checking", "formatting");
    cmd!(sh, "cargo fmt --check").quiet().run()?;

    output::status("Running", "clippy");
    crate::lint::run(sh)?;

    output::status("Running", "tests");
    crate::test::run(sh)?;

    Ok(())
}
