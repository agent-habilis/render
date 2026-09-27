// The public surface is `run` and `mermaid`; everything else stays `pub(crate)`.
pub(crate) mod cli;
pub(crate) mod deck;
pub(crate) mod img;
pub(crate) mod markdown;
pub mod mermaid;
pub(crate) mod table;
pub(crate) mod util;

use std::process::ExitCode;

#[must_use]
pub fn run() -> ExitCode {
    match cli::run_cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            util::output::error(&format!("{error:#}"));
            ExitCode::FAILURE
        }
    }
}
