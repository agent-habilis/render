use std::time::Duration;

use ansi_to_tui::IntoText;
use ratatui::text::Text;

use super::{Align, Cached, Component, FrameCtx, Item, parse_align, tokenize_attrs};

/// Default per-frame duration when `seconds` is absent or unparseable.
const DEFAULT_MS: u32 = 1000;
/// Floor (ms) on per-frame duration, so a tiny/zero `seconds` can't
/// divide-by-zero or spin the render loop; and the ceiling (1 hour).
const MIN_MS: f32 = 50.0;
const MAX_MS: f32 = 3_600_000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnimationComponent {
    pub(crate) frame_ms: u32,
    /// `loop`: wrap back to the first frame forever; else hold the last.
    pub(crate) looping: bool,
    pub(crate) align: Option<Align>,
    /// Each frame verbatim, as preformatted text.
    pub(crate) frames: Vec<String>,
}

impl AnimationComponent {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "seconds*1000 is checked finite and clamped into [50, 3_600_000] before the cast"
    )]
    pub(crate) fn parse(attrs: &str, content: &str) -> Self {
        let mut frame_ms = DEFAULT_MS;
        let mut looping = false;
        for (key, value) in tokenize_attrs(attrs) {
            match key.as_str() {
                "seconds" => {
                    if let Some(secs) = value.as_deref().and_then(|val| val.parse::<f32>().ok())
                        && secs.is_finite()
                    {
                        frame_ms = (secs * 1000.0).clamp(MIN_MS, MAX_MS) as u32;
                    }
                }
                "loop" => looping = value.as_deref() != Some("false"),
                _ => {}
            }
        }
        Self {
            frame_ms,
            looping,
            align: parse_align(attrs),
            frames: parse_frames(content),
        }
    }

    pub(crate) fn frame_index(&self, elapsed: Duration) -> usize {
        let count = self.frames.len();
        if count <= 1 {
            return 0;
        }
        let raw = (elapsed.as_millis() / u128::from(self.frame_ms.max(1))) as usize;
        if self.looping {
            raw % count
        } else {
            raw.min(count - 1)
        }
    }

    /// Whether the animation is still moving at `elapsed` (so the loop keeps
    /// repainting): a looping multi-frame animation always is; a one-shot one
    /// only until it reaches its last frame.
    pub(crate) fn is_animating(&self, elapsed: Duration) -> bool {
        self.frames.len() > 1 && (self.looping || self.frame_index(elapsed) + 1 < self.frames.len())
    }
}

impl Component for AnimationComponent {
    fn render(&self, _content_w: usize) -> Cached {
        Cached::Frames(self.frames.iter().map(|frame| frame_item(frame)).collect())
    }

    fn align(&self) -> Option<Align> {
        self.align
    }

    fn lay_out(&self, ctx: &FrameCtx<'_>) -> Option<Item> {
        let Cached::Frames(frames) = ctx.cached else {
            return None;
        };
        let (text, width, height) = frames.get(self.frame_index(ctx.elapsed))?;
        if *height == 0 {
            return None;
        }
        Some(Item {
            width: (*width).min(ctx.content.width),
            height: *height,
            placement: self.placement(),
            text: Some(text.clone()),
        })
    }

    fn is_active(&self, ctx: &FrameCtx<'_>) -> bool {
        self.is_animating(ctx.elapsed)
    }
}

/// Split an `<animation>` body into its `<frame>` … `</frame>` children, each
/// captured verbatim. Lines outside a frame (blank lines, indentation) are
/// ignored. An unterminated `<frame>` ends the scan.
fn parse_frames(content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos < lines.len() {
        if lines[pos].trim() == "<frame>" {
            let mut close = pos + 1;
            while close < lines.len() && lines[close].trim() != "</frame>" {
                close += 1;
            }
            if close >= lines.len() {
                break;
            }
            frames.push(lines[pos + 1..close].join("\n"));
            pos = close + 1;
        } else {
            pos += 1;
        }
    }
    frames
}

/// A `<frame>` rendered **verbatim** (preformatted; every space kept) to a
/// ratatui [`Text`] with its measured size. Surrounding blank lines are **not**
/// trimmed, so a frame keeps its exact height — the figure stays put across the
/// flip instead of jumping.
fn frame_item(frame: &str) -> (Text<'static>, u16, u16) {
    let width = frame
        .lines()
        .map(console::measure_text_width)
        .max()
        .unwrap_or(0);
    let height = frame.lines().count().max(1);
    let text = frame
        .into_text()
        .unwrap_or_else(|_| Text::raw(frame.to_owned()));
    (
        text,
        u16::try_from(width).unwrap_or(u16::MAX),
        u16::try_from(height).unwrap_or(u16::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::AnimationComponent;
    use std::time::Duration;

    #[test]
    fn parses_seconds_loop_and_verbatim_frames() {
        let component = AnimationComponent::parse(
            " seconds=\"0.5\" loop",
            "<frame>\n  A\n</frame>\n<frame>\nB\nB2\n</frame>",
        );
        assert_eq!(component.frame_ms, 500);
        assert!(component.looping);
        assert_eq!(component.frames, vec!["  A".to_owned(), "B\nB2".to_owned()]);
    }

    #[test]
    fn defaults_to_one_second_no_loop() {
        let component = AnimationComponent::parse("", "<frame>\nx\n</frame>");
        assert_eq!(component.frame_ms, 1000);
        assert!(!component.looping);
    }

    #[test]
    fn frame_index_advances_and_holds_or_loops() {
        let component = AnimationComponent::parse(
            " seconds=\"0.2\"",
            "<frame>\nA\n</frame>\n<frame>\nB\n</frame>\n<frame>\nC\n</frame>",
        );
        assert_eq!(component.frame_index(Duration::ZERO), 0);
        assert_eq!(component.frame_index(Duration::from_millis(250)), 1);
        assert_eq!(component.frame_index(Duration::from_secs(10)), 2);
        assert!(component.is_animating(Duration::ZERO));
        assert!(!component.is_animating(Duration::from_secs(10)));

        let looping = AnimationComponent::parse(
            " seconds=\"0.2\" loop",
            "<frame>\nA\n</frame>\n<frame>\nB\n</frame>",
        );
        assert!(looping.frame_index(Duration::from_secs(10)) < 2);
        assert!(looping.is_animating(Duration::from_secs(10)));
    }
}
