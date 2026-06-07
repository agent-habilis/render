use std::io::{IsTerminal, Write};

/// A cargo-style status line on stderr: a right-aligned bold-green verb (in a
/// 12-column field, matching cargo's `Compiling`/`Finished`/…) then the message,
/// e.g. `   Installing ah-r`. Plain when stderr is not a TTY or `NO_COLOR` is set.
pub(crate) fn status(verb: &str, message: &str) {
    let mut err = std::io::stderr();
    let color = err.is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let _ = if color {
        writeln!(err, "\x1b[1;32m{verb:>12}\x1b[0m {message}")
    } else {
        writeln!(err, "{verb:>12} {message}")
    };
}
