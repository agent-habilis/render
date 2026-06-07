use super::{Cached, Component, measured_text};
use crate::mermaid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MermaidComponent(String);

impl MermaidComponent {
    pub(crate) fn new(src: String) -> Self {
        Self(src)
    }
}

impl From<&str> for MermaidComponent {
    fn from(src: &str) -> Self {
        Self(src.to_owned())
    }
}

impl Component for MermaidComponent {
    fn render(&self, _content_w: usize) -> Cached {
        let rendered = mermaid::mermaid(&self.0).unwrap_or_else(|_| self.0.clone());
        let (text, width, height) = measured_text(&rendered);
        Cached::Static(text, width, height)
    }
}
