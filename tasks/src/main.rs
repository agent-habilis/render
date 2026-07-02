use std::process::ExitCode;

use clap::{Parser, Subcommand};
use xshell::Shell;

mod ci;
mod clean;
mod fmt;
mod install;
mod lint;
mod release;
mod setup;
mod test;
mod util;

/// Task result; any `Err` is printed and turns into a non-zero exit.
pub(crate) type TaskOutcome = Result<(), Box<dyn std::error::Error>>;

/// Project task runner. Run `cargo task <task>`.
#[derive(Parser)]
#[command(bin_name = "cargo task")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

/// Variant doc comments *are* the `--help` text — no separate usage block to
/// drift. clap kebab-cases the names, so the invocation surface stays stable.
#[derive(Subcommand)]
enum Task {
    /// Format source files.
    Fmt,
    /// Run clippy lints.
    Lint,
    /// Run tests.
    Test,
    /// Run the CI gate (fmt check, clippy, tests).
    Ci,
    /// Remove build artifacts.
    Clean,
    /// Install the `ahre` binary.
    Install,
    /// Build the release binary. With a level (`patch`|`minor`|`major`|
    /// `x.y.z`, plus flags like `--execute`) drive cargo-release; dry run by
    /// default.
    Release {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Install the `/render` agent skill (forwards to `ahre setup`; dry run
    /// unless `--execute`). Pass flags after `--`, e.g. `-- --execute`.
    Setup {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Remove the `/render` agent skill (forwards to `ahre teardown`; dry run
    /// unless `--execute`).
    Teardown {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let sh = match Shell::new() {
        Ok(sh) => sh,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let outcome = match cli.task {
        Task::Fmt => fmt::run(&sh),
        Task::Lint => lint::run(&sh),
        Task::Test => test::run(&sh),
        Task::Ci => ci::run(&sh),
        Task::Clean => clean::run(&sh),
        Task::Install => install::run(&sh),
        Task::Release { args } => release::run(&sh, &args),
        Task::Setup { args } => setup::setup(&sh, &args),
        Task::Teardown { args } => setup::teardown(&sh, &args),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
