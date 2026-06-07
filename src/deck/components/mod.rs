//! The renderable pieces a slide is built from. Each component lives in its own
//! module and owns its data, its attribute parsing, and how it renders. They
//! share the [`Component`] trait (pre-render + layout policy), the [`Cached`]
//! render representation, and the [`measured_text`]/[`tokenize_attrs`] helpers.
//! The deck runtime ([`crate::deck`]) dispatches to them via
//! [`crate::deck::Segment`].

use std::time::Duration;

use ansi_to_tui::IntoText;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;

pub(crate) mod animation;
pub(crate) mod code;
pub(crate) mod markdown;
pub(crate) mod mermaid;
pub(crate) mod tty;

pub(crate) use animation::AnimationComponent;
pub(crate) use code::CodeComponent;
pub(crate) use markdown::MarkdownComponent;
pub(crate) use mermaid::MermaidComponent;
pub(crate) use tty::TtyComponent;

use tty::ProcessManager;

/// Everything a component needs to lay itself out, draw, or report activity for
/// one frame. The deck runtime builds one per segment and dispatches the trait
/// methods through it, so it never matches on the concrete component type.
pub(crate) struct FrameCtx<'frame> {
    pub(crate) cached: &'frame Cached,
    pub(crate) manager: &'frame ProcessManager,
    /// Time since the current slide opened — the `<animation>` clock.
    pub(crate) elapsed: Duration,
    pub(crate) current: usize,
    pub(crate) index: usize,
    pub(crate) content: Rect,
    pub(crate) content_w: usize,
    pub(crate) blink_on: bool,
}

/// Live components carry no text and draw themselves from [`FrameCtx`]; static
/// and animation components carry the text to draw.
#[derive(Debug)]
pub(crate) struct Item {
    pub(crate) width: u16,
    pub(crate) height: u16,
    /// Share the left edge of the text column (prose, code) rather than being
    /// centered on its own (diagrams, animations, tty boxes).
    pub(crate) left_align: bool,
    pub(crate) text: Option<Text<'static>>,
}

/// A slide component: it pre-renders itself, lays itself out, draws, and reports
/// whether it still needs repainting. The central loop dispatches through
/// [`crate::deck::Segment::as_component`] and never inspects the
/// concrete type.
pub(crate) trait Component: std::fmt::Debug {
    /// Pre-render to the cached representation at wrap width `content_w`.
    fn render(&self, content_w: usize) -> Cached;

    /// Whether the component shares the left edge of the slide's text column
    /// rather than being centered on its own.
    fn left_align(&self) -> bool {
        false
    }

    /// The default handles a [`Cached::Static`] text render; live and animated
    /// components override it.
    fn lay_out(&self, ctx: &FrameCtx<'_>) -> Option<Item> {
        match ctx.cached {
            Cached::Static(text, width, height) if *height > 0 => Some(Item {
                width: (*width).min(ctx.content.width),
                height: *height,
                left_align: self.left_align(),
                text: Some(text.clone()),
            }),
            Cached::Static(..) | Cached::Live | Cached::Frames(_) => None,
        }
    }

    /// The default paints its text; live components override it to draw from
    /// their runtime state.
    fn draw(&self, frame: &mut Frame<'_>, area: Rect, item: &Item, _ctx: &FrameCtx<'_>) {
        if let Some(text) = &item.text {
            frame.render_widget(Paragraph::new(text.clone()), area);
        }
    }

    /// Whether the component still needs repainting (live output, animation);
    /// the deck runtime keeps ticking while any component is active.
    fn is_active(&self, _ctx: &FrameCtx<'_>) -> bool {
        false
    }
}

/// Cached across repaints of the same slide+width.
#[derive(Debug)]
pub(crate) enum Cached {
    /// Laid out fresh each frame, nothing cached.
    Live,
    /// `(text, width, height)`.
    Static(Text<'static>, u16, u16),
    /// One `(text, width, height)` per `<animation>` frame.
    Frames(Vec<(Text<'static>, u16, u16)>),
}

/// Surrounding blank lines are trimmed so vertical centering is not skewed.
pub(crate) fn measured_text(rendered: &str) -> (Text<'static>, u16, u16) {
    let mut lines: Vec<&str> = rendered.split('\n').collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }

    let width = lines
        .iter()
        .map(|line| console::measure_text_width(line))
        .max()
        .unwrap_or(0);
    let height = lines.len();
    let joined = lines.join("\n");
    let text = match joined.as_str().into_text() {
        Ok(text) => text,
        Err(_) => Text::raw(joined),
    };

    (
        text,
        u16::try_from(width).unwrap_or(u16::MAX),
        u16::try_from(height).unwrap_or(u16::MAX),
    )
}

/// A bare token (no `=`) yields `(key, None)`; `key=value` and
/// `key="quoted value"` yield `(key, Some(value))`. Whitespace separates tokens
/// except inside quotes.
pub(crate) fn tokenize_attrs(attrs: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let chars: Vec<char> = attrs.chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        if chars[idx].is_whitespace() {
            idx += 1;
            continue;
        }
        let key_start = idx;
        while idx < chars.len() && chars[idx] != '=' && !chars[idx].is_whitespace() {
            idx += 1;
        }
        let key: String = chars[key_start..idx].iter().collect();

        if idx < chars.len() && chars[idx] == '=' {
            idx += 1;
            let value = if idx < chars.len() && chars[idx] == '"' {
                idx += 1;
                let val_start = idx;
                while idx < chars.len() && chars[idx] != '"' {
                    idx += 1;
                }
                let value: String = chars[val_start..idx].iter().collect();
                if idx < chars.len() {
                    idx += 1;
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
