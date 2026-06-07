//! Parse an `.ah-r` file into a [`Deck`] of [`Slide`]s.
//!
//! A deck is a Markdown file with embedded XML islands. Standalone `---` lines
//! separate slides; a `<deck title="…" />` / `<slide title="…" />` self-closing
//! tag at the start of a slide carries frontmatter. The XML components —
//! `<code>`, `<tty>`, `<mermaid>`, `<animation>` — express what Markdown cannot;
//! see [`Segment`].
//!
//! Splitting is fence- and component-aware: a `---` inside a code fence or a
//! component is content, not a separator.

use anyhow::{Result, bail};

use super::components::tokenize_attrs;
use super::segment::{
    ComponentKind, Segment, is_close_fence, open_fence_len, split_segments, xml_close, xml_open,
};

#[derive(Debug, Default)]
pub(crate) struct Deck {
    pub(crate) title: Option<String>,
    pub(crate) slides: Vec<Slide>,
}

/// A per-slide `title` overrides the deck title in the footer.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Slide {
    pub(crate) title: Option<String>,
    pub(crate) body: String,
}

impl Slide {
    pub(crate) fn footer_title<'deck>(&'deck self, deck: &'deck Deck) -> Option<&'deck str> {
        self.title.as_deref().or(deck.title.as_deref())
    }

    /// A slide with no component yields a single [`Segment::Markdown`] holding
    /// the whole body.
    pub(crate) fn segments(&self) -> Vec<Segment> {
        split_segments(&self.body)
    }
}

#[derive(Debug, Clone, Copy)]
enum MetaKind {
    /// Applies to the whole deck.
    Deck,
    /// Applies to the current slide.
    Slide,
}

/// A malformed or empty document yields the slides it can (possibly none — the
/// caller treats that as an error).
pub(crate) fn parse_deck(input: &str) -> Result<Deck> {
    let mut deck = Deck::default();
    let mut deck_tags = 0_usize;

    for section in split_slides(input) {
        let (meta_tags, body) = strip_meta_tags(&section);

        let mut slide_title = None;
        for (kind, title) in meta_tags {
            match kind {
                MetaKind::Deck => {
                    deck_tags += 1;
                    if title.is_some() {
                        deck.title = title;
                    }
                }
                MetaKind::Slide => {
                    if title.is_some() {
                        slide_title = title;
                    }
                }
            }
        }

        // A frontmatter-only section (e.g. a top-of-file `<deck/>`) has no body
        // and is not a slide.
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        deck.slides.push(Slide {
            title: slide_title,
            body: body.to_owned(),
        });
    }

    if deck_tags > 1 {
        bail!("a deck may declare at most one <deck> tag, but found {deck_tags}");
    }
    Ok(deck)
}

/// Splits on standalone `---` lines, but fence- and component-aware: while
/// inside a ` ``` ` fence or a component, a `---` line is content.
fn split_slides(input: &str) -> Vec<String> {
    enum State {
        Open,
        Fence(usize),
        Xml(ComponentKind),
    }

    let mut sections = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut state = State::Open;

    for line in input.lines() {
        let trimmed = line.trim();
        match state {
            State::Fence(open_len) => {
                if is_close_fence(trimmed, open_len) {
                    state = State::Open;
                }
                current.push(line);
            }
            State::Xml(component) => {
                if xml_close(trimmed, component) {
                    state = State::Open;
                }
                current.push(line);
            }
            State::Open => {
                if let Some(open_len) = open_fence_len(trimmed) {
                    state = State::Fence(open_len);
                    current.push(line);
                } else if let Some((component, _)) = xml_open(trimmed) {
                    state = State::Xml(component);
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

/// Only tags at the very start (after blank lines) are treated as metadata; a
/// `<deck/>` later in the slide is ordinary Markdown. Each tag is a single line.
fn strip_meta_tags(section: &str) -> (Vec<(MetaKind, Option<String>)>, String) {
    let lines: Vec<&str> = section.lines().collect();
    let mut pos = 0;
    let mut tags = Vec::new();

    loop {
        while pos < lines.len() && lines[pos].trim().is_empty() {
            pos += 1;
        }
        let Some(tag) = lines.get(pos).and_then(|line| meta_tag(line.trim())) else {
            break;
        };
        tags.push(tag);
        pos += 1;
    }

    (tags, lines[pos..].join("\n"))
}

/// The name must be followed by whitespace or the tag's end, and the tag must
/// close with `>` (the `/` is tolerated but optional); other attributes ignored.
fn meta_tag(trimmed: &str) -> Option<(MetaKind, Option<String>)> {
    let rest = trimmed.strip_prefix('<')?;
    let (kind, after) = if let Some(after) = rest.strip_prefix("deck") {
        (MetaKind::Deck, after)
    } else if let Some(after) = rest.strip_prefix("slide") {
        (MetaKind::Slide, after)
    } else {
        return None;
    };
    // The name must end here (a boundary), so `<deckx/>` is not read as `deck`.
    if !(after.starts_with(char::is_whitespace) || after.starts_with('/') || after.starts_with('>'))
    {
        return None;
    }
    let inner = after.trim_end().strip_suffix('>')?;
    let attrs = inner.strip_suffix('/').unwrap_or(inner);
    let title = tokenize_attrs(attrs)
        .into_iter()
        .find(|(key, _)| key == "title")
        .and_then(|(_, value)| value);
    Some((kind, title))
}

#[cfg(test)]
mod tests {
    use super::{Segment, parse_deck};
    use super::super::components::AnimationComponent;

    fn animation(segment: &Segment) -> &AnimationComponent {
        match segment {
            Segment::Animation(component) => component,
            Segment::Markdown(_) | Segment::Tty(_) | Segment::Code(_) | Segment::Mermaid(_) => {
                panic!("expected an animation segment")
            }
        }
    }

    #[test]
    fn separator_inside_a_frame_is_not_a_split() {
        let deck = parse_deck("<animation>\n<frame>\n---\n</frame>\n</animation>\n\n---\n\n# Next")
            .unwrap();
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(animation(&deck.slides[0].segments()[0]).frames, vec!["---".to_owned()]);
        assert_eq!(deck.slides[1].body, "# Next");
    }

    #[test]
    fn separator_inside_a_component_is_not_a_split() {
        let deck = parse_deck("# Slide\n\n<code lang=\"yaml\">\nfoo: 1\n---\nbar: 2\n</code>\n\n---\n\n# Next")
            .unwrap();
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(
            deck.slides[0].body,
            "# Slide\n\n<code lang=\"yaml\">\nfoo: 1\n---\nbar: 2\n</code>"
        );
        assert_eq!(deck.slides[1].body, "# Next");
    }

    #[test]
    fn splits_on_separators_without_frontmatter() {
        let deck = parse_deck("# One\n\n---\n\n# Two\n\n---\n\n# Three").unwrap();
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
            parse_deck("<deck title=\"My Talk\" author=\"Caio\" />\n\n# Intro\n\n---\n\n# End")
                .unwrap();
        assert_eq!(deck.title.as_deref(), Some("My Talk"));
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].body, "# Intro");
        assert_eq!(deck.slides[1].body, "# End");
    }

    #[test]
    fn separator_inside_a_code_fence_is_not_a_split() {
        let deck = parse_deck("# Slide\n\n```yaml\nfoo: 1\n---\nbar: 2\n```\n\n---\n\n# Next")
            .unwrap();
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(
            deck.slides[0].body,
            "# Slide\n\n```yaml\nfoo: 1\n---\nbar: 2\n```"
        );
        assert_eq!(deck.slides[1].body, "# Next");
    }

    #[test]
    fn per_slide_title_overrides_deck_title() {
        let input = "<deck title=\"Deck\" />\n\n# First\n\n---\n\n<slide title=\"Second Page\" />\n\n# Second";
        let deck = parse_deck(input).unwrap();
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].footer_title(&deck), Some("Deck"));
        assert_eq!(deck.slides[1].footer_title(&deck), Some("Second Page"));
        assert_eq!(deck.slides[1].body, "# Second");
    }

    #[test]
    fn deck_block_alone_is_not_a_slide() {
        let deck = parse_deck("<deck title=\"T\" />\n\n---\n\n# Slide").unwrap();
        assert_eq!(deck.title.as_deref(), Some("T"));
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].body, "# Slide");
        assert_eq!(deck.slides[0].footer_title(&deck), Some("T"));
    }

    #[test]
    fn more_than_one_deck_tag_is_an_error() {
        let result = parse_deck("<deck title=\"A\" />\n\n# One\n\n---\n\n<deck title=\"B\" />\n\n# Two");
        assert!(result.is_err());
    }

    #[test]
    fn single_slide_no_separators() {
        let deck = parse_deck("# Only\n\nsome body text").unwrap();
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].body, "# Only\n\nsome body text");
    }

    #[test]
    fn blank_input_has_no_slides() {
        assert!(parse_deck("   \n\n  ").unwrap().slides.is_empty());
    }

    #[test]
    fn slide_without_title_falls_back_to_none() {
        let deck = parse_deck("# Slide").unwrap();
        assert_eq!(deck.slides[0].footer_title(&deck), None);
    }

    #[test]
    fn separator_always_splits_even_with_prose() {
        let deck = parse_deck("# A\n\n---\n\nplain text\n\n---\n\n# B").unwrap();
        let bodies: Vec<&str> = deck
            .slides
            .iter()
            .map(|slide| slide.body.as_str())
            .collect();
        assert_eq!(bodies, ["# A", "plain text", "# B"]);
    }
}
