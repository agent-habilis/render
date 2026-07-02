use std::fmt::Write as _;
use std::path::Path;

use crossterm::style::{Attribute, SetAttribute, SetForegroundColor};

use super::{BLUE, FG};

// Zero-width space: survives termimad and counts as width 0 in its wrapper, so a
// marked link text wraps exactly as its bare text would. `extract` fences each
// link text between a pair of these; `apply` swaps the pairs for OSC-8 links
// once termimad has done its width-aware layout.
const MARK: char = '\u{200b}';

/// Replace each inline `[text](url)` with `MARK text MARK`, returning the
/// rewritten source and the resolved URLs in source order. Sentinels are
/// width-0, so termimad still wraps on the visible link text.
pub(super) fn extract(input: &str, base: Option<&Path>) -> (String, Vec<String>) {
    let mut out = String::with_capacity(input.len());
    let mut urls = Vec::new();
    let bytes = input.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // A `[` starts a link unless it's the alt-text of an image (`![`).
        if bytes[pos] == b'['
            && !(pos > 0 && bytes[pos - 1] == b'!')
            && let Some((text, url, end)) = parse_link(input, pos)
        {
            out.push(MARK);
            out.push_str(text);
            out.push(MARK);
            urls.push(resolve(url, base));
            pos = end;
            continue;
        }
        let ch = input[pos..].chars().next().unwrap();
        out.push(ch);
        pos += ch.len_utf8();
    }

    (out, urls)
}

/// Parse `[text](url)` starting at the `[` at `open`. Returns the text, url, and
/// the byte index just past the closing `)`. Rejects a text or url that runs
/// across a newline (not a well-formed inline link).
fn parse_link(input: &str, open: usize) -> Option<(&str, &str, usize)> {
    let rest = &input[open + 1..];
    let close_text = rest.find(']')?;
    let text = &rest[..close_text];
    if text.contains('\n') {
        return None;
    }
    let after = &rest[close_text + 1..];
    if !after.starts_with('(') {
        return None;
    }
    let close_url = after.find(')')?;
    let url = &after[1..close_url];
    if url.contains('\n') || url.is_empty() {
        return None;
    }
    let end = open + 1 + close_text + 1 + close_url + 1;
    Some((text, url, end))
}

fn resolve(url: &str, base: Option<&Path>) -> String {
    if has_scheme(url) {
        return url.to_owned();
    }
    let Some(base) = base else {
        return url.to_owned();
    };
    let Ok(abs) = std::path::absolute(base.join(url)) else {
        return url.to_owned();
    };
    format!("file://{}", abs.display().to_string().replace(' ', "%20"))
}

/// Whether `url` carries an explicit scheme (`https://`, `mailto:`, `file:`, …)
/// — the leading run of scheme chars up to the first `:` per RFC 3986. A bare
/// path like `./security.md` or `docs/x.md` has none.
fn has_scheme(url: &str) -> bool {
    let Some(colon) = url.find(':') else {
        return false;
    };
    let scheme = &url[..colon];
    !scheme.is_empty()
        && scheme.starts_with(|first: char| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

/// Swap each `MARK … MARK` span for an OSC-8 hyperlink whose visible text is the
/// marked text styled blue + underlined, consuming `urls` in order. Trailing
/// text after the link is restored to whatever SGR was active before it.
pub(super) fn apply(rendered: &str, urls: &[String]) -> String {
    if urls.is_empty() {
        return rendered.to_owned();
    }

    let link_open = format!(
        "{}{}",
        SetForegroundColor(BLUE),
        SetAttribute(Attribute::Underlined)
    );
    let underline_off = SetAttribute(Attribute::NoUnderline);
    let mut out = String::with_capacity(rendered.len());
    let mut active_sgr = SetForegroundColor(FG).to_string();
    let mut urls = urls.iter();
    let mut chars = rendered.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if ch != MARK {
            out.push(ch);
            if ch == '\x1b'
                && let Some(seq) = sgr_at(rendered, idx)
            {
                seq.clone_into(&mut active_sgr);
            }
            continue;
        }

        // A `MARK` with no matching close on the same visible run (wrapped link
        // text) is dropped without consuming a url, keeping indices aligned.
        let Some(text) = marked_text(rendered, idx + MARK.len_utf8()) else {
            continue;
        };
        let Some(url) = urls.next() else {
            continue;
        };
        write!(
            out,
            "\x1b]8;;{url}\x1b\\{link_open}{text}{underline_off}{active_sgr}\x1b]8;;\x1b\\"
        )
        .expect("writing to a String is infallible");
        // Skip past the text and its closing MARK.
        for _ in 0..=text.chars().count() {
            chars.next();
        }
    }

    out
}

/// The text between the MARK at `start` and the next MARK, or `None` if the
/// closing MARK doesn't appear before a newline (the pair got wrapped).
fn marked_text(rendered: &str, start: usize) -> Option<&str> {
    let rest = &rendered[start..];
    let close = rest.find(MARK)?;
    let text = &rest[..close];
    if text.contains('\n') {
        return None;
    }
    Some(text)
}

/// The full SGR sequence (`\x1b[…m`) beginning at the ESC at `esc`, if any.
fn sgr_at(haystack: &str, esc: usize) -> Option<&str> {
    let rest = &haystack[esc..];
    if !rest.starts_with("\x1b[") {
        return None;
    }
    let end = rest.find('m')?;
    let params = &rest[2..end];
    params
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b';')
        .then_some(&rest[..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drop SGR and OSC-8 escapes, leaving only what the user actually sees.
    fn visible(rendered: &str) -> String {
        let mut out = String::new();
        let mut chars = rendered.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '\x1b' {
                out.push(ch);
                continue;
            }
            match chars.peek() {
                Some('[') => {
                    for esc in chars.by_ref() {
                        if esc == 'm' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC-8: run to the ST (`\x1b\\`).
                    while let Some(esc) = chars.next() {
                        if esc == '\x1b' {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    #[test]
    fn extract_marks_text_and_collects_urls() {
        let (out, urls) = extract("see [a](http://x) and [b](http://y)", None);
        assert_eq!(out, "see \u{200b}a\u{200b} and \u{200b}b\u{200b}");
        assert_eq!(urls, ["http://x", "http://y"]);
    }

    #[test]
    fn image_links_are_left_alone() {
        let (out, urls) = extract("![alt](pic.png)", None);
        assert_eq!(out, "![alt](pic.png)");
        assert!(urls.is_empty());
    }

    #[test]
    fn relative_target_resolves_to_file_url() {
        let (_, urls) = extract("[x](./security.md)", Some(Path::new("/docs")));
        assert_eq!(urls, ["file:///docs/security.md"]);
    }

    #[test]
    fn absolute_scheme_passes_through() {
        let (_, urls) = extract("[x](https://a.b/c)", Some(Path::new("/docs")));
        assert_eq!(urls, ["https://a.b/c"]);
    }

    #[test]
    fn apply_emits_osc8_and_hides_url_and_brackets() {
        let (marked, urls) = extract("[security.md](./security.md)", Some(Path::new("/docs")));
        let out = apply(&marked, &urls);
        assert!(out.contains("\x1b]8;;file:///docs/security.md\x1b\\"));
        // Nothing markdown-syntactic survives in the visible text.
        assert_eq!(visible(&out), "security.md");
    }

    #[test]
    fn apply_restores_prior_color_after_link() {
        let marked = format!("\x1b[38;2;1;2;3mpre {MARK}txt{MARK} post");
        let out = apply(&marked, &["http://x".to_owned()]);
        // The active color is re-emitted after the link so `post` isn't left blue.
        assert!(out.contains("\x1b[24m\x1b[38;2;1;2;3m\x1b]8;;\x1b\\ post"));
    }
}
