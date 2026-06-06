//! Parse an `.ahr-deck` file into a [`Deck`] of [`Slide`]s.
//!
//! A deck is a **Markdown file with embedded XML islands**. It is split into
//! slides on standalone `---` lines (`---` is *only* a slide separator), and
//! deck/slide metadata lives in fenced code blocks at the start of a slide:
//!
//! - ```` ```deck ```` — deck-level frontmatter (`title`), usually at the top of
//!   the file.
//! - ```` ```deck:slide ```` — per-slide frontmatter; its `title` overrides the
//!   deck title in that slide's footer.
//!
//! Everything else is ordinary Markdown, except three embedded XML elements that
//! express things Markdown cannot — see [`Segment`]:
//!
//! - `<code lang="…" render="…">…</code>` — syntax-highlighted code.
//! - `<tty …>…</tty>` — a live command in a pseudo-terminal.
//! - `<mermaid>…</mermaid>` — a drawn diagram.
//!
//! Plain ` ``` ` fences are just Markdown code blocks. Splitting is fence- and
//! element-aware, so a `---` inside a fence or an element is content, not a
//! separator. The scanners are hand-rolled in the same line-by-line style.

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
    /// with the embedded `<code>` / `<tty>` / `<mermaid>` elements. A slide with
    /// no element yields a single [`Segment::Markdown`] holding the whole body.
    pub(crate) fn segments(&self) -> Vec<Segment> {
        split_segments(&self.body)
    }
}

/// One piece of a slide, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// A run of Markdown, rendered plainly (a ` ``` ` fence is just a code block).
    Markdown(String),
    /// A `<tty>` element: a command to run live in a boxed pseudo-terminal.
    Tty(TtyBlock),
    /// A `<code>` element: code rendered with real syntax highlighting by an
    /// editor (see [`super::tty`]).
    Code(CodeBlock),
    /// A `<mermaid>` element: its source, drawn as a diagram.
    Mermaid(String),
}

/// A `<code>` element: its `lang`, the code body, and when it is rendered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CodeBlock {
    /// `lang` — the language token (e.g. `rust`, `html`, `js`) for highlighting.
    pub(crate) lang: String,
    /// The code between the tags.
    pub(crate) code: String,
    /// When the highlighting editor is spawned (`render` attribute).
    pub(crate) render: RenderMode,
}

/// When a code block's syntax-highlighting editor is spawned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RenderMode {
    /// Spawn on first view of the slide (`render="lazy"`).
    #[default]
    Lazy,
    /// Spawn at deck load and keep in memory (`render="eager"`).
    Eager,
}

/// A `<tty>` element: its attributes plus the command body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TtyBlock {
    /// `id` — links the same logical process across slides (for `persist`).
    pub(crate) id: Option<String>,
    /// `persist` — keep the process alive across a slide change when the next
    /// slide has a `<tty>` with the same `id`.
    pub(crate) persist: bool,
    /// `title` — box title; falls back to the command's first token.
    pub(crate) title: Option<String>,
    /// `status` — fixed status text; falls back to the live process status.
    pub(crate) status: Option<String>,
    /// `rows` — box interior height; a default is used when absent.
    pub(crate) rows: Option<u16>,
    /// `cols` — box interior width; a default is used when absent.
    pub(crate) cols: Option<u16>,
    /// The shell command/script between the tags.
    pub(crate) command: String,
}

impl TtyBlock {
    /// The box title: the `title` override, else the command's first token
    /// (its first whitespace-delimited word on the first line), else `"tty"`.
    pub(crate) fn display_title(&self) -> &str {
        if let Some(title) = &self.title {
            return title;
        }
        self.command.split_whitespace().next().unwrap_or("tty")
    }
}

/// An embedded XML element kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Element {
    Code,
    Tty,
    Mermaid,
}

impl Element {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "code" => Some(Self::Code),
            "tty" => Some(Self::Tty),
            "mermaid" => Some(Self::Mermaid),
            _ => None,
        }
    }

    fn close_tag(self) -> &'static str {
        match self {
            Self::Code => "</code>",
            Self::Tty => "</tty>",
            Self::Mermaid => "</mermaid>",
        }
    }
}

/// Split a slide body into [`Segment`]s. Markdown fences are opaque — kept whole
/// as Markdown so a `<code>`-looking line *inside* a code sample is never parsed
/// as an element. Between fences, a `<code>` / `<tty>` / `<mermaid>` opening tag
/// starts an element that runs to its closing tag; an unterminated element is
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

        // A Markdown code fence is opaque: keep it (and its contents) verbatim.
        if let Some(open_len) = open_fence_len(trimmed) {
            let mut close = pos + 1;
            while close < lines.len() && !is_close_fence(lines[close].trim(), open_len) {
                close += 1;
            }
            let end = close.min(lines.len() - 1);
            for line in &lines[pos..=end] {
                markdown.push(line);
            }
            pos = end + 1;
            continue;
        }

        // An embedded XML element.
        if let Some((element, attrs)) = xml_open(trimmed) {
            let mut close = pos + 1;
            while close < lines.len() && !xml_close(lines[close].trim(), element) {
                close += 1;
            }
            if close < lines.len() {
                flush(&mut markdown, &mut segments);
                let content = lines[pos + 1..close].join("\n");
                segments.push(match element {
                    Element::Code => Segment::Code(parse_code_attrs(attrs, content)),
                    Element::Tty => Segment::Tty(parse_tty_attrs(attrs, content)),
                    Element::Mermaid => Segment::Mermaid(content),
                });
                pos = close + 1;
                continue;
            }
            // Unterminated element: fall through and treat the line as Markdown.
        }

        markdown.push(lines[pos]);
        pos += 1;
    }

    flush(&mut markdown, &mut segments);
    segments
}

/// Recognise an opening tag line — `<code …>`, `<tty …>`, or `<mermaid …>` —
/// returning the element and the attribute substring between the tag name and
/// the closing `>`. The whole opening tag must be on the one (trimmed) line and
/// end with `>`; `<codex>`/`<code-block>` and the like are rejected.
fn xml_open(trimmed: &str) -> Option<(Element, &str)> {
    let rest = trimmed.strip_prefix('<')?;
    let name_end = rest
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let element = Element::from_name(&rest[..name_end])?;
    let after = rest[name_end..].trim_end();
    let attrs = after.strip_suffix('>')?;
    let attrs = attrs.strip_suffix('/').unwrap_or(attrs);
    // The name must be followed by a boundary (whitespace or the `>`), so a
    // longer name like `code-block` is not matched as `code`.
    if !attrs.is_empty() && !attrs.starts_with(char::is_whitespace) {
        return None;
    }
    Some((element, attrs))
}

/// Whether `trimmed` is the closing tag for `element` (e.g. `</code>`).
fn xml_close(trimmed: &str, element: Element) -> bool {
    trimmed == element.close_tag()
}

/// Parse a `<code>` element's attributes (`lang`, `render`) plus its body.
fn parse_code_attrs(attrs: &str, code: String) -> CodeBlock {
    let tokens = tokenize_attrs(attrs);
    let lang = tokens
        .iter()
        .find(|(key, _)| key == "lang")
        .and_then(|(_, value)| value.clone())
        .unwrap_or_default();
    let render = tokens
        .iter()
        .find(|(key, _)| key == "render")
        .and_then(|(_, value)| value.as_deref())
        .map_or(RenderMode::Lazy, |value| match value {
            "eager" => RenderMode::Eager,
            _ => RenderMode::Lazy,
        });
    CodeBlock { lang, code, render }
}

/// Parse a `<tty>` element's attributes into a [`TtyBlock`]. Attributes are
/// whitespace-separated `key="value"` pairs or bare flags (e.g. `persist`);
/// unknown keys are ignored.
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

/// Tokenize an attribute string into `(key, value)` pairs. A bare token (no `=`)
/// yields `(key, None)`; `key=value` and `key="quoted value"` yield
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
/// Fence- and element-aware: while inside a ` ``` ` fence or a `<code>` /
/// `<tty>` / `<mermaid>` element, a `---` line is content, not a separator.
fn split_slides(input: &str) -> Vec<String> {
    /// What opaque block the splitter is currently inside.
    enum State {
        Open,
        Fence(usize),
        Xml(Element),
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
            State::Xml(element) => {
                if xml_close(trimmed, element) {
                    state = State::Open;
                }
                current.push(line);
            }
            State::Open => {
                if let Some(open_len) = open_fence_len(trimmed) {
                    state = State::Fence(open_len);
                    current.push(line);
                } else if let Some((element, _)) = xml_open(trimmed) {
                    state = State::Xml(element);
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
    use super::{RenderMode, Segment, parse_deck, split_segments};

    fn tty(segment: &Segment) -> &super::TtyBlock {
        match segment {
            Segment::Tty(block) => block,
            Segment::Markdown(_) | Segment::Code(_) | Segment::Mermaid(_) => {
                panic!("expected a tty segment")
            }
        }
    }

    fn code(segment: &Segment) -> &super::CodeBlock {
        match segment {
            Segment::Code(block) => block,
            Segment::Markdown(_) | Segment::Tty(_) | Segment::Mermaid(_) => {
                panic!("expected a code segment")
            }
        }
    }

    #[test]
    fn body_without_elements_is_a_single_markdown_segment() {
        let segs = split_segments("# Title\n\nsome **text**");
        assert_eq!(segs, vec![Segment::Markdown("# Title\n\nsome **text**".into())]);
    }

    #[test]
    fn tty_element_splits_markdown_around_it() {
        let body = "# Demo\n\n<tty>\ncounter\n</tty>\n\nafter";
        let segs = split_segments(body);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], Segment::Markdown("# Demo\n".into()));
        assert_eq!(tty(&segs[1]).command, "counter");
        assert_eq!(segs[2], Segment::Markdown("\nafter".into()));
    }

    #[test]
    fn tty_attributes_are_parsed_with_quotes_and_flags() {
        let body = "<tty id=\"counter\" persist title=\"Live counter\" rows=\"12\" cols=\"40\">\nwhile :; do echo hi; done\n</tty>";
        let block = tty(&split_segments(body)[0]).clone();
        assert_eq!(block.id.as_deref(), Some("counter"));
        assert!(block.persist);
        assert_eq!(block.title.as_deref(), Some("Live counter"));
        assert_eq!(block.rows, Some(12));
        assert_eq!(block.cols, Some(40));
        assert_eq!(block.command, "while :; do echo hi; done");
    }

    #[test]
    fn tty_defaults_when_attributes_absent() {
        let block = tty(&split_segments("<tty>\nhtop\n</tty>")[0]).clone();
        assert!(block.id.is_none());
        assert!(!block.persist);
        assert!(block.title.is_none());
        assert_eq!(block.display_title(), "htop");
    }

    #[test]
    fn code_element_carries_lang_and_render() {
        let segs = split_segments("# Title\n\n<code lang=\"rust\">\nfn main() {}\n</code>");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], Segment::Markdown("# Title\n".into()));
        let block = code(&segs[1]);
        assert_eq!(block.lang, "rust");
        assert_eq!(block.code, "fn main() {}");
        assert_eq!(block.render, RenderMode::Lazy); // default
    }

    #[test]
    fn render_attribute_selects_eager_or_lazy() {
        let eager = "<code lang=\"rust\" render=\"eager\">\nx\n</code>";
        let lazy = "<code lang=\"rust\" render=\"lazy\">\nx\n</code>";
        let bogus = "<code lang=\"rust\" render=\"bogus\">\nx\n</code>";
        assert_eq!(code(&split_segments(eager)[0]).render, RenderMode::Eager);
        assert_eq!(code(&split_segments(lazy)[0]).render, RenderMode::Lazy);
        assert_eq!(code(&split_segments(bogus)[0]).render, RenderMode::Lazy);
    }

    #[test]
    fn mermaid_element_becomes_a_mermaid_segment() {
        let segs = split_segments("<mermaid>\nflowchart LR; A --> B\n</mermaid>");
        assert_eq!(segs, vec![Segment::Mermaid("flowchart LR; A --> B".into())]);
    }

    #[test]
    fn unterminated_element_stays_markdown() {
        let body = "<code lang=\"rust\">\nfn main() {}\nno closing tag";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn code_shown_inside_a_fence_is_not_an_element() {
        // A `<code>` inside a Markdown code sample stays Markdown.
        let body = "```html\n<code lang=\"rust\">x</code>\n```";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn plain_fences_stay_markdown() {
        // ```mermaid and ```rust fences are now plain code, not special blocks.
        for body in ["```\nplain\n```", "```rust\nfn main() {}\n```", "```mermaid\nA-->B\n```"] {
            assert_eq!(split_segments(body), vec![Segment::Markdown(body.into())]);
        }
    }

    #[test]
    fn looks_like_element_but_longer_name_is_markdown() {
        let body = "<codex>\nx\n</codex>";
        assert_eq!(split_segments(body), vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn separator_inside_an_element_is_not_a_split() {
        // A `---` inside a `<code>` block is code, not a slide separator.
        let deck = parse_deck("# Slide\n\n<code lang=\"yaml\">\nfoo: 1\n---\nbar: 2\n</code>\n\n---\n\n# Next");
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(
            deck.slides[0].body,
            "# Slide\n\n<code lang=\"yaml\">\nfoo: 1\n---\nbar: 2\n</code>"
        );
        assert_eq!(deck.slides[1].body, "# Next");
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
