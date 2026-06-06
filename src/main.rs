//! Thin binary shim. All CLI logic lives in the library
//! ([`agent_habilis_render::run_cli`]); `main` owns only the process exit
//! code. The tool is a pure stdin -> stdout text filter, so there is no
//! async runtime or terminal state to manage here.

use anyhow::Result;

fn main() -> Result<()> {
    agent_habilis_render::run_cli()
}
