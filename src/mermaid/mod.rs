use std::panic::{self, AssertUnwindSafe};

use anyhow::{Context, Result, anyhow};

pub(crate) fn mermaid(input: &str) -> Result<String> {
    // figurehead panics rather than erroring on some inputs, so one bad diagram
    // would abort the whole render. Contain it — hook silenced so the backtrace
    // doesn't reach the terminal — letting callers fall back to the raw source.
    // (Only unwinds under a panic=unwind profile; the vendored self-message fix
    // in vendor/figurehead is what keeps discovery.md's diagrams from panicking.)
    let prev = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(|| figurehead::render(input)));
    panic::set_hook(prev);

    match result {
        Ok(render) => render.context("rendering mermaid diagram"),
        Err(_) => Err(anyhow!("mermaid renderer panicked on this diagram")),
    }
}
