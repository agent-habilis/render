//! Slide segments: splitting a slide body into its embedded components and the
//! Markdown runs around them.

use super::components::{
    AnimationComponent, CodeComponent, Component, MarkdownComponent, MermaidComponent, TtyComponent,
};

/// One piece of a slide, in source order. Each variant wraps the
/// [`Component`](super::components::Component) that owns its data, parsing, and
/// rendering (see [`super::components`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    Markdown(MarkdownComponent),
    Tty(TtyComponent),
    Code(CodeComponent),
    Mermaid(MermaidComponent),
    Animation(AnimationComponent),
}

impl Segment {
    pub(crate) fn as_component(&self) -> &dyn Component {
        match self {
            Segment::Markdown(component) => component,
            Segment::Tty(component) => component,
            Segment::Code(component) => component,
            Segment::Mermaid(component) => component,
            Segment::Animation(component) => component,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ComponentKind {
    Code,
    Tty,
    Mermaid,
    Animation,
}

impl ComponentKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "code" => Some(Self::Code),
            "tty" => Some(Self::Tty),
            "mermaid" => Some(Self::Mermaid),
            "animation" => Some(Self::Animation),
            _ => None,
        }
    }

    fn close_tag(self) -> &'static str {
        match self {
            Self::Code => "</code>",
            Self::Tty => "</tty>",
            Self::Mermaid => "</mermaid>",
            Self::Animation => "</animation>",
        }
    }
}

/// Split a slide body into [`Segment`]s. Markdown fences are opaque — kept whole
/// as Markdown so a `<code>`-looking line inside a code sample is never parsed as
/// a component. An opening tag starts a component that runs to its closing tag;
/// an unterminated component is left as Markdown rather than swallowing the rest
/// of the slide.
pub(super) fn split_segments(body: &str) -> Vec<Segment> {
    let lines: Vec<&str> = body.lines().collect();
    let mut segments = Vec::new();
    let mut markdown: Vec<&str> = Vec::new();
    let mut pos = 0;

    let flush = |md: &mut Vec<&str>, out: &mut Vec<Segment>| {
        if md.iter().any(|line| !line.trim().is_empty()) {
            out.push(Segment::Markdown(MarkdownComponent::new(md.join("\n"))));
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

        if let Some((component, attrs)) = xml_open(trimmed) {
            let mut close = pos + 1;
            while close < lines.len() && !xml_close(lines[close].trim(), component) {
                close += 1;
            }
            if close < lines.len() {
                flush(&mut markdown, &mut segments);
                let content = lines[pos + 1..close].join("\n");
                segments.push(match component {
                    ComponentKind::Code => Segment::Code(CodeComponent::parse(attrs, content)),
                    ComponentKind::Tty => Segment::Tty(TtyComponent::parse(attrs, content)),
                    ComponentKind::Mermaid => Segment::Mermaid(MermaidComponent::new(content)),
                    ComponentKind::Animation => {
                        Segment::Animation(AnimationComponent::parse(attrs, &content))
                    }
                });
                pos = close + 1;
                continue;
            }
            // Unterminated component: fall through and treat the line as Markdown.
        }

        markdown.push(lines[pos]);
        pos += 1;
    }

    flush(&mut markdown, &mut segments);
    segments
}

/// Recognise an opening tag line — `<code …>`, `<tty …>`, or `<mermaid …>` —
/// returning the component and the attribute substring between the tag name and
/// the closing `>`. The whole opening tag must be on the one (trimmed) line and
/// end with `>`; `<codex>`/`<code-component>` and the like are rejected.
pub(super) fn xml_open(trimmed: &str) -> Option<(ComponentKind, &str)> {
    let rest = trimmed.strip_prefix('<')?;
    let name_end = rest
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let component = ComponentKind::from_name(&rest[..name_end])?;
    let after = rest[name_end..].trim_end();
    let attrs = after.strip_suffix('>')?;
    let attrs = attrs.strip_suffix('/').unwrap_or(attrs);
    // The name must be followed by a boundary (whitespace or the `>`), so a
    // longer name like `code-component` is not matched as `code`.
    if !attrs.is_empty() && !attrs.starts_with(char::is_whitespace) {
        return None;
    }
    Some((component, attrs))
}

pub(super) fn xml_close(trimmed: &str, component: ComponentKind) -> bool {
    trimmed == component.close_tag()
}

pub(super) fn open_fence_len(trimmed: &str) -> Option<usize> {
    let len = trimmed.bytes().take_while(|&byte| byte == b'`').count();
    (len >= 3).then_some(len)
}

pub(super) fn is_close_fence(trimmed: &str, min_len: usize) -> bool {
    trimmed.len() >= min_len && trimmed.bytes().all(|byte| byte == b'`')
}

#[cfg(test)]
mod tests {
    use super::{Segment, split_segments};

    fn tty(segment: &Segment) -> &super::TtyComponent {
        match segment {
            Segment::Tty(component) => component,
            Segment::Markdown(_) | Segment::Code(_) | Segment::Mermaid(_) | Segment::Animation(_) => {
                panic!("expected a tty segment")
            }
        }
    }

    fn code(segment: &Segment) -> &super::CodeComponent {
        match segment {
            Segment::Code(component) => component,
            Segment::Markdown(_) | Segment::Tty(_) | Segment::Mermaid(_) | Segment::Animation(_) => {
                panic!("expected a code segment")
            }
        }
    }

    fn animation(segment: &Segment) -> &super::AnimationComponent {
        match segment {
            Segment::Animation(component) => component,
            Segment::Markdown(_) | Segment::Tty(_) | Segment::Code(_) | Segment::Mermaid(_) => {
                panic!("expected an animation segment")
            }
        }
    }

    #[test]
    fn body_without_components_is_a_single_markdown_segment() {
        let segs = split_segments("# Title\n\nsome **text**");
        assert_eq!(segs, vec![Segment::Markdown("# Title\n\nsome **text**".into())]);
    }

    #[test]
    fn tty_component_splits_markdown_around_it() {
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
        let component = tty(&split_segments(body)[0]).clone();
        assert_eq!(component.id.as_deref(), Some("counter"));
        assert!(component.persist);
        assert_eq!(component.title.as_deref(), Some("Live counter"));
        assert_eq!(component.rows, Some(12));
        assert_eq!(component.cols, Some(40));
        assert_eq!(component.command, "while :; do echo hi; done");
    }

    #[test]
    fn tty_defaults_when_attributes_absent() {
        let component = tty(&split_segments("<tty>\nhtop\n</tty>")[0]).clone();
        assert!(component.id.is_none());
        assert!(!component.persist);
        assert!(component.title.is_none());
        assert_eq!(component.display_title(), "htop");
    }

    #[test]
    fn code_component_carries_lang() {
        let segs = split_segments("# Title\n\n<code lang=\"rust\">\nfn main() {}\n</code>");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], Segment::Markdown("# Title\n".into()));
        let component = code(&segs[1]);
        assert_eq!(component.lang, "rust");
        assert_eq!(component.code, "fn main() {}");
    }

    #[test]
    fn tty_prerender_flag_is_parsed() {
        assert!(tty(&split_segments("<tty prerender>\nhtop\n</tty>")[0]).prerender);
        assert!(
            !tty(&split_segments("<tty prerender=\"false\">\nhtop\n</tty>")[0]).prerender
        );
        assert!(!tty(&split_segments("<tty>\nhtop\n</tty>")[0]).prerender);
    }

    #[test]
    fn tty_keys_attribute_is_captured_raw() {
        let body = "<tty keys=\"\\r:q\\r\">\nnvim file\n</tty>";
        let component = tty(&split_segments(body)[0]).clone();
        // Stored verbatim (escapes decoded later, by the tty layer).
        assert_eq!(component.keys.as_deref(), Some("\\r:q\\r"));
    }

    #[test]
    fn mermaid_component_becomes_a_mermaid_segment() {
        let segs = split_segments("<mermaid>\nflowchart LR; A --> B\n</mermaid>");
        assert_eq!(segs, vec![Segment::Mermaid("flowchart LR; A --> B".into())]);
    }

    #[test]
    fn unterminated_component_stays_markdown() {
        let body = "<code lang=\"rust\">\nfn main() {}\nno closing tag";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn code_shown_inside_a_fence_is_not_a_component() {
        let body = "```html\n<code lang=\"rust\">x</code>\n```";
        let segs = split_segments(body);
        assert_eq!(segs, vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn plain_fences_stay_markdown() {
        for body in ["```\nplain\n```", "```rust\nfn main() {}\n```", "```mermaid\nA-->B\n```"] {
            assert_eq!(split_segments(body), vec![Segment::Markdown(body.into())]);
        }
    }

    #[test]
    fn looks_like_component_but_longer_name_is_markdown() {
        let body = "<codex>\nx\n</codex>";
        assert_eq!(split_segments(body), vec![Segment::Markdown(body.into())]);
    }

    #[test]
    fn animation_parses_frames_seconds_and_loop() {
        let body = "<animation seconds=\"0.5\" loop>\n<frame>\nA\n</frame>\n<frame>\nB\nB2\n</frame>\n</animation>";
        let component = animation(&split_segments(body)[0]).clone();
        assert_eq!(component.frame_ms, 500);
        assert!(component.looping);
        assert_eq!(component.frames, vec!["A".to_owned(), "B\nB2".to_owned()]);
    }

    #[test]
    fn animation_defaults_and_preserves_frame_indentation() {
        let body = "<animation>\n<frame>\n  ___\n / o \\\n</frame>\n</animation>";
        let component = animation(&split_segments(body)[0]).clone();
        assert_eq!(component.frame_ms, 1000);
        assert!(!component.looping);
        assert_eq!(component.frames, vec!["  ___\n / o \\".to_owned()]);
    }

    #[test]
    fn unterminated_animation_stays_markdown() {
        let body = "<animation>\n<frame>\nA\n</frame>";
        assert_eq!(split_segments(body), vec![Segment::Markdown(body.into())]);
    }
}
