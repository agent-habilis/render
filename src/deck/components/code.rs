use super::{Align, Cached, Component, measured_text, parse_align, tokenize_attrs};
use crate::markdown;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CodeComponent {
    pub(crate) lang: String,
    pub(crate) align: Option<Align>,
    pub(crate) code: String,
}

impl CodeComponent {
    pub(crate) fn parse(attrs: &str, code: String) -> Self {
        let lang = tokenize_attrs(attrs)
            .into_iter()
            .find(|(key, _)| key == "lang")
            .and_then(|(_, value)| value)
            .unwrap_or_default();
        Self {
            lang,
            align: parse_align(attrs),
            code,
        }
    }
}

impl Component for CodeComponent {
    fn render(&self, _content_w: usize) -> Cached {
        let (text, width, height) = measured_text(&markdown::render_code(&self.lang, &self.code));
        Cached::Static(text, width, height)
    }

    fn left_align(&self) -> bool {
        true
    }

    fn align(&self) -> Option<Align> {
        self.align
    }
}

#[cfg(test)]
mod tests {
    use super::CodeComponent;

    #[test]
    fn lang_defaults_to_empty() {
        assert_eq!(CodeComponent::parse("", "x".into()).lang, "");
    }

    #[test]
    fn lang_attribute_is_read() {
        let component = CodeComponent::parse(" lang=\"rust\"", "fn main() {}".into());
        assert_eq!(component.lang, "rust");
        assert_eq!(component.code, "fn main() {}");
    }
}
