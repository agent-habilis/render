use super::{Align, Cached, Component, measured_text, parse_align};
use crate::mermaid;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MermaidComponent {
    src: String,
    align: Option<Align>,
}

impl MermaidComponent {
    pub(crate) fn new(src: String) -> Self {
        Self { src, align: None }
    }

    pub(crate) fn parse(attrs: &str, src: String) -> Self {
        Self {
            src,
            align: parse_align(attrs),
        }
    }
}

impl From<&str> for MermaidComponent {
    fn from(src: &str) -> Self {
        Self::new(src.to_owned())
    }
}

impl Component for MermaidComponent {
    fn render(&self, _content_w: usize) -> Cached {
        let rendered = mermaid::mermaid(&self.src).unwrap_or_else(|_| self.src.clone());
        let (text, width, height) = measured_text(&rendered);
        Cached::Static(text, width, height)
    }

    fn align(&self) -> Option<Align> {
        self.align
    }
}
