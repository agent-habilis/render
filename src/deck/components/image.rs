use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use super::{Align, Cached, Component, FrameCtx, Item, parse_align};
use crate::deck::Segment;
use crate::img::{draw, fit_aspect, load, scale};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ImageComponent {
    pub(crate) src: String,
    /// Target display size in terminal cells; the image is scaled (up or down) to
    /// it, preserving aspect ratio. Giving one derives the other from the image's
    /// aspect; giving neither uses the natural size. Always clamped to the slide.
    pub(crate) width: Option<u16>,
    pub(crate) height: Option<u16>,
    pub(crate) align: Option<Align>,
    /// Shown in place of the image when it can't be loaded.
    pub(crate) alt: Option<String>,
    /// `alt`, or a default message naming `src` — resolved once at parse so the
    /// per-frame layout/draw of a broken image neither reallocates nor risks the
    /// measured box and the drawn text drifting apart.
    fallback: String,
}

impl ImageComponent {
    pub(crate) fn parse(attrs: &str) -> Self {
        let mut component = Self::default();
        for (key, value) in super::tokenize_attrs(attrs) {
            match key.as_str() {
                "src" => component.src = value.unwrap_or_default(),
                "width" => component.width = value.and_then(|val| val.parse().ok()),
                "height" => component.height = value.and_then(|val| val.parse().ok()),
                "alt" => component.alt = value,
                _ => {}
            }
        }
        component.align = parse_align(attrs);
        component.fallback = component
            .alt
            .clone()
            .unwrap_or_else(|| format!("⚠ cannot load image: {}", component.src));
        component
    }

    fn fallback(&self) -> &str {
        &self.fallback
    }

    /// The cell box to draw the image in: the `width`/`height` target (or natural
    /// size), then clamped to the slide's content box (shrink-only) so it never
    /// overflows.
    fn fit(&self, natural: Size, ctx: &FrameCtx<'_>) -> (u16, u16) {
        let bound_w = u16::try_from(ctx.content_w)
            .unwrap_or(u16::MAX)
            .min(ctx.content.width);
        let target = target_size(natural, self.width, self.height);
        let fitted = fit_aspect(target, bound_w, ctx.content.height, false);
        (fitted.width, fitted.height)
    }
}

/// The requested display size from `width`/`height`, always preserving the
/// image's aspect ratio (scaling up or down): one given derives the other; both
/// given fits the image within that box (so the laid-out box matches what
/// [`draw`] renders); neither falls back to the natural size.
fn target_size(natural: Size, width: Option<u16>, height: Option<u16>) -> Size {
    let (nat_w, nat_h) = (natural.width.max(1), natural.height.max(1));
    match (width, height) {
        (Some(width), Some(height)) => fit_aspect(natural, width, height, true),
        (Some(width), None) => {
            let width = width.max(1);
            Size::new(width, scale(width, nat_h, nat_w))
        }
        (None, Some(height)) => {
            let height = height.max(1);
            Size::new(scale(height, nat_w, nat_h), height)
        }
        (None, None) => Size::new(nat_w, nat_h),
    }
}

impl Component for ImageComponent {
    // Drawn live from the `ImageStore`; nothing to pre-render.
    fn render(&self, _content_w: usize) -> Cached {
        Cached::Live
    }

    fn align(&self) -> Option<Align> {
        self.align
    }

    fn lay_out(&self, ctx: &FrameCtx<'_>) -> Option<Item> {
        let (width, height) = if let Some(natural) = ctx.images.natural(ctx.current, ctx.index) {
            self.fit(natural, ctx)
        } else {
            // Failed or not-yet-loaded: an alt/error box, wrapped so a long
            // message (e.g. a long path) is not truncated to one line.
            let text_w = u16::try_from(console::measure_text_width(self.fallback())).unwrap_or(u16::MAX);
            let width = text_w.min(ctx.content.width).max(1);
            let height = text_w.div_ceil(width).min(ctx.content.height.max(1)).max(1);
            (width, height)
        };
        Some(Item {
            width,
            height,
            placement: self.placement(),
            text: None,
        })
    }

    fn draw(&self, frame: &mut Frame<'_>, area: Rect, _item: &Item, ctx: &FrameCtx<'_>) {
        if !ctx.images.draw_image(ctx.current, ctx.index, frame, area) {
            frame.render_widget(Paragraph::new(self.fallback()).wrap(Wrap { trim: false }), area);
        }
    }
}

/// A decoded image and its render protocol.
struct Ready {
    natural: Size,
    // Boxed: `StatefulProtocol` is large, and most decks have few images.
    protocol: Box<StatefulProtocol>,
}

/// Owns every decoded image and its render protocol, keyed by `(slide, segment
/// index)`. Decoded once by [`Self::ensure`] (eagerly at deck load) and never
/// evicted — decks are small. A map value of `None` records a failed decode, so
/// it isn't retried; "failed" and "absent" are deliberately equivalent at the
/// read sites. Interior mutability keeps the deck runtime's `&` draw path simple,
/// the way the `<tty>` `ProcessManager` does.
pub(crate) struct ImageStore {
    picker: Picker,
    base_dir: PathBuf,
    loaded: RefCell<HashMap<(usize, usize), Option<Ready>>>,
}

impl ImageStore {
    pub(crate) fn new(picker: Picker, base_dir: PathBuf) -> Self {
        Self {
            picker,
            base_dir,
            loaded: RefCell::new(HashMap::new()),
        }
    }

    /// Drop every decoded image so a reload re-decodes with fresh content (the
    /// `(slide, index)` keys would otherwise serve a stale image once edits shift
    /// slides).
    pub(crate) fn clear(&self) {
        self.loaded.borrow_mut().clear();
    }

    /// Decode (once) every `<img>` on slide `current` that isn't loaded yet.
    pub(crate) fn ensure(&self, current: usize, segments: &[Segment]) {
        let mut loaded = self.loaded.borrow_mut();
        for (index, segment) in segments.iter().enumerate() {
            let Segment::Image(component) = segment else {
                continue;
            };
            let key = (current, index);
            if loaded.contains_key(&key) {
                continue;
            }
            let entry = self
                .load(&component.src)
                .ok()
                .map(|(natural, protocol)| Ready {
                    natural,
                    protocol: Box::new(protocol),
                });
            loaded.insert(key, entry);
        }
    }

    fn load(&self, src: &str) -> Result<(Size, StatefulProtocol)> {
        load(&self.picker, &self.resolve(src))
    }

    fn resolve(&self, src: &str) -> PathBuf {
        let path = Path::new(src);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        }
    }

    /// The image's natural cell size, or `None` if it failed to load or isn't
    /// loaded yet (the component then lays out an alt box).
    pub(crate) fn natural(&self, slide: usize, index: usize) -> Option<Size> {
        match self.loaded.borrow().get(&(slide, index)) {
            Some(Some(ready)) => Some(ready.natural),
            Some(None) | None => None,
        }
    }

    /// Draw the loaded image into `area`; returns `false` (so the caller draws the
    /// alt text) when there is no image to draw.
    pub(crate) fn draw_image(
        &self,
        slide: usize,
        index: usize,
        frame: &mut Frame<'_>,
        area: Rect,
    ) -> bool {
        let mut loaded = self.loaded.borrow_mut();
        match loaded.get_mut(&(slide, index)) {
            Some(Some(ready)) => {
                draw(frame, area, ready.protocol.as_mut());
                true
            }
            Some(None) | None => false,
        }
    }
}

impl std::fmt::Debug for ImageStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `StatefulProtocol` is not `Debug`; summarise instead.
        formatter
            .debug_struct("ImageStore")
            .field("base_dir", &self.base_dir)
            .field("loaded", &self.loaded.borrow().len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageComponent, target_size};
    use ratatui::layout::Size;

    #[test]
    fn target_size_always_preserves_aspect() {
        let natural = Size::new(100, 50); // 2:1
        // width-only derives height; upscales past natural.
        assert_eq!(target_size(natural, Some(200), None), Size::new(200, 100));
        // height-only derives width.
        assert_eq!(target_size(natural, None, Some(10)), Size::new(20, 10));
        // both given: the image is fit (aspect-kept) inside the box, not stretched
        // to it — a 30x30 box on a 2:1 image yields a 30x15 box.
        assert_eq!(target_size(natural, Some(30), Some(30)), Size::new(30, 15));
        assert_eq!(target_size(natural, None, None), natural);
    }

    #[test]
    fn parses_src_and_attrs() {
        let component =
            ImageComponent::parse(" src=\"assets/logo.png\" width=\"40\" height=\"20\" alt=\"a logo\"");
        assert_eq!(component.src, "assets/logo.png");
        assert_eq!(component.width, Some(40));
        assert_eq!(component.height, Some(20));
        assert_eq!(component.alt.as_deref(), Some("a logo"));
        assert_eq!(component.fallback(), "a logo");
    }

    #[test]
    fn defaults_when_only_src_present() {
        let component = ImageComponent::parse(" src=\"pic.jpg\"");
        assert_eq!(component.src, "pic.jpg");
        assert!(component.width.is_none());
        assert!(component.height.is_none());
        assert!(component.alt.is_none());
        assert_eq!(component.fallback(), "⚠ cannot load image: pic.jpg");
    }
}
