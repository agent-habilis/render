use console::style;

// `console` strips color when stderr isn't a terminal, so piped/captured
// output (an agent running `ah-r …`) stays clean.
pub(crate) fn status(verb: &str, msg: &str) {
    eprintln!("{} {msg}", style(format!("{verb:>12}")).green().bold());
}

pub(crate) fn warn(msg: &str) {
    eprintln!("{}: {msg}", style("warning").yellow().bold());
}

pub(crate) fn error(msg: &str) {
    eprintln!("{}: {msg}", style("error").red().bold());
}
