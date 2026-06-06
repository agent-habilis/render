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

    /// Split the slide body into ordered segments: runs of Markdown interleaved
    /// with ` ```tty ` execution blocks. A slide with no `tty` block yields a
    /// single [`Segment::Markdown`] holding the whole body.
    pub(crate) fn segments(&self) -> Vec<Segment> {
        split_segments(&self.body)
    }
}

/// One piece of a slide, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// A run of Markdown (rendered with `render::markdown_to_width`, so embedded
    /// ` ```mermaid ` diagrams are still drawn inline).
    Markdown(String),
    /// A ` ```tty ` block: a command to run live in a boxed pseudo-terminal.
    Tty(TtyBlock),
    /// A language-tagged code fence (e.g. ` ```rust `): rendered with real
    /// syntax highlighting by an editor (see [`super::tty`]).
    Code(CodeBlock),
}

/// A language-tagged code fence: its info-string language and the code body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CodeBlock {
    /// The fence's info-string language token (e.g. `rust`, `html`, `js`).
    pub(crate) lang: String,
    /// The code between the fences.
    pub(crate) code: String,
    /// When the highlighting editor is spawned (`render=` attribute).
    pub(crate) render: RenderMode,
}

/// When a code block's syntax-highlighting editor is spawned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RenderMode {
    /// Spawn on first view of the slide (`render=lazy`).
    #[default]
    Lazy,
    /// Spawn at deck load and keep in memory (`render=eager`).
    Eager,
}

/// A parsed ` ```tty ` block: its fence attributes plus the command body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TtyBlock {
    /// `id=` — links the same logical process across slides (for `persist`).
    pub(crate) id: Option<String>,
    /// `persist` — keep the process alive across a slide change when the next
    /// slide has a `tty` block with the same `id`.
    pub(crate) persist: bool,
    /// `title=` — box title; falls back to the command's first token.
    pub(crate) title: Option<String>,
    /// `status=` — fixed status text; falls back to the live process status.
    pub(crate) status: Option<String>,
    /// `rows=` — box interior height; a default is used when absent.
    pub(crate) rows: Option<u16>,
    /// `cols=` — box interior width; a default is used when absent.
    pub(crate) cols: Option<u16>,
    /// The shell command/script between the fences.
    pub(crate) command: String,
}

impl TtyBlock {
    /// The box title: the `title=` override, else the command's first token
    /// (its first whitespace-delimited word on the first line), else `"tty"`.
    pub(crate) fn display_title(&self) -> &str {
        if let Some(title) = &self.title {
            return title;
        }
        self.command
            .split_whitespace()
            .next()
            .unwrap_or("tty")
    }
}

/// Split a slide body into [`Segment`]s. Fence-aware in the same hand-rolled
/// style as [`split_slides`]: a ` ```tty ` opening fence starts a block that
/// runs to the next closing fence; any other fenced block (```` ```rust ````,
/// ` ```mermaid `, ...) is passed through verbatim as Markdown so a `tty`-like
/// line inside it is never mistaken for a block. An unterminated `tty` fence is
/// left as Markdown rather than swallowing the rest of the slide.
fn split_segments(body: &str) -> Vec<Segment> {
    let lines: Vec<&str> = body.lines().collect();
    let mut segments = Vec::new();
    let mut markdown: Vec<&str> = Vec::new();
    let mut pos = 0;

    let flush = |md: &mut Vec<&str>, out: &mut Vec<Segment>| {
        if md.iter().any(|line| !line.trim().is_empty()) {
            out.push(Segment::Markdown(md.join("\n")));
        }
        md.clear();
    };

    while pos < lines.len() {
        let trimmed = lines[pos].trim();
        let Some(open_len) = open_fence_len(trimmed) else {
            markdown.push(lines[pos]);
            pos += 1;
            continue;
        };

        // Find the matching closing fence for any opening fence.
        let mut close = pos + 1;
        while close < lines.len() && !is_close_fence(lines[close].trim(), open_len) {
            close += 1;
        }
        let terminated = close < lines.len();

        if terminated && is_tty_open_fence(trimmed) {
            flush(&mut markdown, &mut segments);
            let attrs = trimmed.strip_prefix("```").unwrap_or("").trim();
            let attrs = attrs.strip_prefix("tty").unwrap_or("");
            let command = lines[pos + 1..close].join("\n");
            segments.push(Segment::Tty(parse_tty_attrs(attrs, command)));
            pos = close + 1;
        } else if let (true, Some((lang, render))) = (terminated, code_fence_meta(trimmed)) {
            // A language-tagged fence (```rust, ```html, ...): syntax-highlight it.
            flush(&mut markdown, &mut segments);
            let code = lines[pos + 1..close].join("\n");
            segments.push(Segment::Code(CodeBlock { lang, code, render }));
            pos = close + 1;
        } else if terminated {
            // A plain (untagged) or ```mermaid fence: keep it whole in Markdown.
            for line in &lines[pos..=close] {
                markdown.push(line);
            }
            pos = close + 1;
        } else {
            // Unterminated fence: leave it (and the rest) as Markdown.
            markdown.push(lines[pos]);
            pos += 1;
        }
    }

    flush(&mut markdown, &mut segments);
    segments
}

/// A ` ```tty ` opening fence: a `` ``` `` prefix whose info string's first
/// token is `tty` (case-insensitive), so attributes may follow it.
fn is_tty_open_fence(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("```")
        .and_then(|info| info.split_whitespace().next())
        .is_some_and(|word| word.eq_ignore_ascii_case("tty"))
}

/// The language token and render mode of a code fence we should
/// syntax-highlight. Returns `None` when the info string is empty (a plain code
/// block, left to Markdown) or its first word is `tty`/`mermaid` (handled
/// elsewhere). The `render=` attribute selects the mode (default [`Lazy`]).
///
/// [`Lazy`]: RenderMode::Lazy
fn code_fence_meta(trimmed: &str) -> Option<(String, RenderMode)> {
    let info = trimmed.strip_prefix("```")?;
    let tokens = tokenize_attrs(info);
    let (lang, _) = tokens.first()?;
    let lower = lang.to_ascii_lowercase();
    if lower == "tty" || lower == "mermaid" {
        return None;
    }
    let render = tokens
        .iter()
        .skip(1)
        .find(|(key, _)| key == "render")
        .and_then(|(_, value)| value.as_deref())
        .map_or(RenderMode::Lazy, |value| match value {
            "eager" => RenderMode::Eager,
            _ => RenderMode::Lazy,
        });
    Some((lang.clone(), render))
}

/// Parse a `tty` fence's attribute string into a [`TtyBlock`]. Attributes are
/// whitespace-separated `key=value` pairs (values may be `"quoted"`) or bare
/// flags; unknown keys are ignored.
fn parse_tty_attrs(attrs: &str, command: String) -> TtyBlock {
    let mut block = TtyBlock {
        command,
        ..TtyBlock::default()
    };
    for (key, value) in tokenize_attrs(attrs) {
        match key.as_str() {
            "id" => block.id = value,
            "persist" => block.persist = value.as_deref() != Some("false"),
            "title" => block.title = value,
            "status" => block.status = value,
            "rows" => block.rows = value.and_then(|val| val.parse().ok()),
            "cols" => block.cols = value.and_then(|val| val.parse().ok()),
            _ => {}
        }
    }
    block
}

/// Tokenize a fence attribute string into `(key, value)` pairs. A bare token
/// (no `=`) yields `(key, None)`; `key=value` and `key="quoted value"` yield
/// `(key, Some(value))`. Whitespace separates tokens except inside quotes.
fn tokenize_attrs(attrs: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let chars: Vec<char> = attrs.chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        if chars[idx].is_whitespace() {
            idx += 1;
            continue;
        }
        // Read the key up to '=' or whitespace.
        let key_start = idx;
        while idx < chars.len() && chars[idx] != '=' && !chars[idx].is_whitespace() {
            idx += 1;
        }
        let key: String = chars[key_start..idx].iter().collect();

        if idx < chars.len() && chars[idx] == '=' {
            idx += 1; // consume '='
            let value = if idx < chars.len() && chars[idx] == '"' {
                idx += 1; // consume opening quote
                let val_start = idx;
                while idx < chars.len() && chars[idx] != '"' {
                    idx += 1;
                }
                let value: String = chars[val_start..idx].iter().collect();
                if idx < chars.len() {
                    idx += 1; // consume closing quote
                }
                value
            } else {
                let val_start = idx;
                while idx < chars.len() && !chars[idx].is_whitespace() {
                    idx += 1;
                }
                chars[val_start..idx].iter().collect()
            };
            out.push((key, Some(value)));
        } else {
            out.push((key, None));
        }
    }

    out
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
    use super::{Segment, parse_deck, split_segments};

    fn tty(segment: &Segment) -> &super::TtyBlock {
        match segment {
            Segment::Tty(block) => block,
            Segment::Markdown(_) | Segment::Code(_) => panic!("expected a tty segment"),
        }
    }

    #[test]
    fn body_without_tty_is_a_single_markdown_segment() {
        let segs = split_segments("# Title\n\nsome **text**");
        assert_eq!(segs, vec![Segment::Markdown("# Title\n\nsome **text**".into())]);
    }

    #[test]
    fn tty_block_splits_markdown_around_it() {
        let body = "# Demo\n\n```tty\ncounter\n```\n\nafter";
        let segs = split_segments(body);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], Segment::Markdown("# Demo\n".into()));
        assert_eq!(tty(&segs[1]).command, "counter");
        assert_eq!(segs[2], Segment::Markdown("\nafter".into()));
    }

    #[test]
    fn tty_attributes_are_parsed_with_quotes_and_flags() {
        let body = "```tty id=counter persist title=\"Live counter\" rows=12 cols=40\nwhile :; do echo hi; done\n```";
        let block = match &split_segments(body)[0] {
            Segment::Tty(block) => block.clone(),
            Segment::Markdown(_) | Segment::Code(_) => panic!("expected tty"),
        };
        assert_eq!(block.id.as_deref(), Some("counter"));
        assert!(block.persist);
        assert_eq!(block.title.as_deref(), Some("Live counter"));
        assert_eq!(block.rows, Some(12));
        assert_eq!(block.cols, Some(40));
        assert_eq!(block.command, "while :; do echo hi; done");
    }

    #[test]
    fn tty_defaults_when_attributes_absent() {
        let block = match &split_segments("```tty\nhtop\n```")[0] {
            Segment::Tty(block) => block.clone(),
            Segment::Markdown(_) | Segment::Code(_) => panic!("expected tty"),
        };
        assert!(block.id.is_none());
        assert!(!block.persist);
        assert!(block.title.is_none());
        assert_eq!(block.display_title(), "htop");
    }

    #[test]
    fn unterminated_tty_fence_stays_markdown() {
        let body = "```tty\ncounter\nno closing fence";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn mermaid_fence_is_kept_whole_as_markdown() {
        // ```mermaid is drawn by the markdown renderer, not extracted as code.
        let body = "```mermaid\ngraph TD; tty-->x\n```";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn plain_untagged_fence_stays_markdown() {
        // No info-string language -> a plain code block, left to termimad.
        let body = "```\nplain code\n```";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    fn code(segment: &Segment) -> &super::CodeBlock {
        match segment {
            Segment::Code(block) => block,
            Segment::Markdown(_) | Segment::Tty(_) => panic!("expected a code segment"),
        }
    }

    #[test]
    fn language_tagged_fence_becomes_a_code_segment() {
        use super::RenderMode;
        let body = "# Title\n\n```rust\nfn main() {}\n```";
        let segs = split_segments(body);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], Segment::Markdown("# Title\n".into()));
        let block = code(&segs[1]);
        assert_eq!(block.lang, "rust");
        assert_eq!(block.code, "fn main() {}");
        assert_eq!(block.render, RenderMode::Lazy); // default
    }

    #[test]
    fn render_attribute_selects_eager_or_lazy() {
        use super::RenderMode;
        assert_eq!(code(&split_segments("```rust render=eager\nx\n```")[0]).render, RenderMode::Eager);
        assert_eq!(code(&split_segments("```rust render=lazy\nx\n```")[0]).render, RenderMode::Lazy);
        assert_eq!(code(&split_segments("```rust render=bogus\nx\n```")[0]).render, RenderMode::Lazy);
    }

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
