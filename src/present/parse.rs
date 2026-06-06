//! Parse an `.ahr-deck` file into a [`Deck`] of [`Slide`]s.
//!
//! The format is Markdown split into slides on standalone `---` lines — `---`
//! is *only* a slide separator. Metadata lives in fenced code blocks at the
//! start of a slide:
//!
//! - ```` ```deck ```` — deck-level frontmatter (`title`), usually at the top of
//!   the file.
//! - ```` ```deck:slide ```` — per-slide frontmatter; its `title` overrides the
//!   deck title in that slide's footer.
//!
//! Each block holds `key: value` lines and is stripped from the rendered body.
//! Splitting is fence-aware, so a `---` inside a fenced code block in slide
//! content is content, not a separator. The scanner is hand-rolled in the same
//! line-by-line style as `render::mermaid_blocks`.

/// A parsed presentation: deck-level metadata plus the ordered slides.
#[derive(Debug, Default)]
pub(crate) struct Deck {
    pub(crate) title: Option<String>,
    pub(crate) slides: Vec<Slide>,
}

/// A single slide: its raw Markdown body and an optional per-slide title that
/// overrides the deck title in the footer.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Slide {
    pub(crate) title: Option<String>,
    pub(crate) body: String,
}

impl Slide {
    /// The title shown in the footer for this slide: the per-slide override if
    /// present, otherwise the deck title (possibly `None`).
    pub(crate) fn footer_title<'deck>(&'deck self, deck: &'deck Deck) -> Option<&'deck str> {
        self.title.as_deref().or(deck.title.as_deref())
    }
}

/// Which scope a leading metadata fence configures.
#[derive(Debug, Clone, Copy)]
enum MetaKind {
    /// A ```` ```deck ```` block: applies to the whole deck.
    Deck,
    /// A ```` ```deck:slide ```` block: applies to the current slide.
    Slide,
}

/// Parse a deck. Infallible: a malformed or empty document simply yields the
/// slides it can (possibly none — the caller decides that is an error).
pub(crate) fn parse_deck(input: &str) -> Deck {
    let mut deck = Deck::default();

    for section in split_slides(input) {
        let (blocks, body) = strip_meta_blocks(&section);

        let mut slide_title = None;
        for (kind, title) in blocks {
            if title.is_none() {
                continue;
            }
            match kind {
                MetaKind::Deck => deck.title = title,
                MetaKind::Slide => slide_title = title,
            }
        }

        // A section that held only frontmatter (e.g. a top-of-file `deck`
        // block) has no body and is not a slide.
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        deck.slides.push(Slide {
            title: slide_title,
            body: body.to_owned(),
        });
    }

    deck
}

/// Split the document into sections on standalone `---` lines.
///
/// Fence-aware: while inside a ```` ``` ````-fenced code block, a `---` line is
/// content (it belongs to the code), not a slide separator. The fence is closed
/// by a line of at least as many back-ticks and nothing else.
fn split_slides(input: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut fence: Option<usize> = None;
    for line in input.lines() {
        let trimmed = line.trim();
        match fence {
            Some(open_len) => {
                if is_close_fence(trimmed, open_len) {
                    fence = None;
                }
                current.push(line);
            }
            None => {
                if let Some(open_len) = open_fence_len(trimmed) {
                    fence = Some(open_len);
                    current.push(line);
                } else if trimmed == "---" {
                    sections.push(current.join("\n"));
                    current.clear();
                } else {
                    current.push(line);
                }
            }
        }
    }
    sections.push(current.join("\n"));
    sections
}

/// The length of a leading run of three or more back-ticks (an opening fence),
/// or `None` for any other line.
fn open_fence_len(trimmed: &str) -> Option<usize> {
    let len = trimmed.bytes().take_while(|&byte| byte == b'`').count();
    (len >= 3).then_some(len)
}

/// Peel leading ```` ```deck ```` / ```` ```deck:slide ```` fenced blocks off a
/// section, returning the parsed blocks (in order) and the remaining body.
///
/// Only blocks at the very start (after blank lines) are treated as metadata; a
/// fence later in the slide is ordinary Markdown. An unterminated fence is left
/// in the body rather than swallowing the rest of the slide.
fn strip_meta_blocks(section: &str) -> (Vec<(MetaKind, Option<String>)>, String) {
    let lines: Vec<&str> = section.lines().collect();
    let mut pos = 0;
    let mut blocks = Vec::new();

    loop {
        while pos < lines.len() && lines[pos].trim().is_empty() {
            pos += 1;
        }
        let Some(kind) = lines.get(pos).and_then(|line| meta_fence_kind(line.trim())) else {
            break;
        };
        let open_len = open_fence_len(lines[pos].trim()).unwrap_or(3);

        let mut close = pos + 1;
        while close < lines.len() && !is_close_fence(lines[close].trim(), open_len) {
            close += 1;
        }
        if close >= lines.len() {
            break; // unterminated fence: leave it (and the rest) as body
        }

        blocks.push((kind, parse_title(&lines[pos + 1..close])));
        pos = close + 1;
    }

    (blocks, lines[pos..].join("\n"))
}

/// Recognise a metadata opening fence: ```` ```deck ```` or
/// ```` ```deck:slide ```` (info string matched after the back-ticks).
fn meta_fence_kind(trimmed: &str) -> Option<MetaKind> {
    match trimmed.strip_prefix("```")?.trim() {
        "deck" => Some(MetaKind::Deck),
        "deck:slide" => Some(MetaKind::Slide),
        _ => None,
    }
}

/// A closing fence: a line of `min_len` or more back-ticks and nothing else.
fn is_close_fence(trimmed: &str, min_len: usize) -> bool {
    trimmed.len() >= min_len && trimmed.bytes().all(|byte| byte == b'`')
}

/// The `title` from the `key: value` lines inside a frontmatter block, if any.
/// Other keys (and non-`key: value` lines) are ignored.
fn parse_title(lines: &[&str]) -> Option<String> {
    lines.iter().find_map(|line| {
        let (key, value) = split_key_value(line)?;
        (key == "title").then(|| value.to_owned())
    })
}

/// Split a `key: value` line. Returns `None` unless the text before the first
/// colon is a simple identifier (letters/digits/`_`/`-`, starting with a
/// letter), so a stray Markdown line never looks like a field.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (raw_key, raw_value) = line.split_once(':')?;
    let key = raw_key.trim();
    if !is_identifier(key) {
        return None;
    }
    Some((key, raw_value.trim()))
}

/// A simple frontmatter key: an ASCII letter followed by letters, digits,
/// underscores, or hyphens.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

#[cfg(test)]
mod tests {
    use super::parse_deck;

    #[test]
    fn splits_on_separators_without_frontmatter() {
        let deck = parse_deck("# One\n\n---\n\n# Two\n\n---\n\n# Three");
        assert!(deck.title.is_none());
        let bodies: Vec<&str> = deck
            .slides
            .iter()
            .map(|slide| slide.body.as_str())
            .collect();
        assert_eq!(bodies, ["# One", "# Two", "# Three"]);
    }

    #[test]
    fn reads_deck_frontmatter_and_ignores_unknown_keys() {
        let deck =
            parse_deck("```deck\ntitle: My Talk\nauthor: Caio\n```\n\n# Intro\n\n---\n\n# End");
        assert_eq!(deck.title.as_deref(), Some("My Talk"));
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].body, "# Intro");
        assert_eq!(deck.slides[1].body, "# End");
    }

    #[test]
    fn separator_inside_a_code_fence_is_not_a_split() {
        // The `---` lives inside a fenced code block, so it is content.
        let deck = parse_deck("# Slide\n\n```yaml\nfoo: 1\n---\nbar: 2\n```\n\n---\n\n# Next");
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(
            deck.slides[0].body,
            "# Slide\n\n```yaml\nfoo: 1\n---\nbar: 2\n```"
        );
        assert_eq!(deck.slides[1].body, "# Next");
    }

    #[test]
    fn per_slide_title_overrides_deck_title() {
        let input = "```deck\ntitle: Deck\n```\n\n# First\n\n---\n\n```deck:slide\ntitle: Second Page\n```\n\n# Second";
        let deck = parse_deck(input);
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].footer_title(&deck), Some("Deck"));
        assert_eq!(deck.slides[1].footer_title(&deck), Some("Second Page"));
        assert_eq!(deck.slides[1].body, "# Second");
    }

    #[test]
    fn deck_block_alone_is_not_a_slide() {
        let deck = parse_deck("```deck\ntitle: T\n```\n\n---\n\n# Slide");
        assert_eq!(deck.title.as_deref(), Some("T"));
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].body, "# Slide");
        assert_eq!(deck.slides[0].footer_title(&deck), Some("T"));
    }

    #[test]
    fn single_slide_no_separators() {
        let deck = parse_deck("# Only\n\nsome body text");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].body, "# Only\n\nsome body text");
    }

    #[test]
    fn blank_input_has_no_slides() {
        assert!(parse_deck("   \n\n  ").slides.is_empty());
    }

    #[test]
    fn slide_without_title_falls_back_to_none() {
        let deck = parse_deck("# Slide");
        assert_eq!(deck.slides[0].footer_title(&deck), None);
    }

    #[test]
    fn separator_always_splits_even_with_prose() {
        let deck = parse_deck("# A\n\n---\n\nplain text\n\n---\n\n# B");
        let bodies: Vec<&str> = deck
            .slides
            .iter()
            .map(|slide| slide.body.as_str())
            .collect();
        assert_eq!(bodies, ["# A", "plain text", "# B"]);
    }
}
