//! The default prose component: a run of Markdown, rendered plainly (a ` ``` `
//! fence is just a styled code block — diagrams come from `<mermaid>`).

use super::{Cached, Component, measured_text};
use crate::markdown;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkdownComponent(String);

impl MarkdownComponent {
    pub(crate) fn new(src: String) -> Self {
        Self(src)
    }
}

impl From<&str> for MarkdownComponent {
    fn from(src: &str) -> Self {
        Self(src.to_owned())
    }
}

impl Component for MarkdownComponent {
    fn render(&self, content_w: usize) -> Cached {
        let (text, width, height) = measured_text(&markdown::markdown_plain(&self.0, content_w));
        Cached::Static(text, width, height)
    }

    fn left_align(&self) -> bool {
        true
    }
}
