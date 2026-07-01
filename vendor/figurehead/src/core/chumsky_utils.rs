//! Shared chumsky parser utilities for diagram parsing
//!
//! This module provides common parser combinators used across different
//! diagram type parsers.

use chumsky::prelude::*;
use chumsky::text::whitespace;

/// Parse optional whitespace including newlines.
///
/// Uses explicit character matching to avoid the "repeated combinator making no progress"
/// issue that can occur with `chumsky::text::whitespace().repeated()`.
pub fn optional_whitespace<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    one_of(" \t\n\r").repeated().ignored()
}

/// Parse required whitespace (at least one whitespace/newline character).
///
/// Uses explicit character matching to avoid the "repeated combinator making no progress"
/// issue.
pub fn whitespace_required<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    one_of(" \t\n\r").repeated().at_least(1).ignored()
}

/// Parse inline whitespace only (spaces and tabs, no newlines).
///
/// Useful for relationship parsers that should not consume statement separators.
pub fn inline_whitespace<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    just(' ').or(just('\t')).repeated().ignored()
}

/// Parse a Mermaid-style comment (%% to end of line).
pub fn mermaid_comment<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    just("%%").ignore_then(none_of('\n').repeated()).ignored()
}

/// Parse optional whitespace or Mermaid comments.
///
/// This is the standard whitespace parser for Mermaid-compatible syntax
/// that supports %% comments.
pub fn whitespace_or_comment<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    whitespace().or(mermaid_comment()).ignored()
}

/// Parse optional sequence of whitespace/comments.
///
/// More permissive than `optional_whitespace` - allows interleaved comments.
pub fn optional_whitespace_or_comment<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    whitespace_or_comment().or_not().ignored()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optional_whitespace() {
        let parser = optional_whitespace().then(just("test")).then_ignore(end());
        assert!(parser.parse("test").into_result().is_ok());
        assert!(parser.parse(" test").into_result().is_ok());
        assert!(parser.parse("  test").into_result().is_ok());
        assert!(parser.parse("\ntest").into_result().is_ok());
        assert!(parser.parse("\t\n test").into_result().is_ok());
    }

    #[test]
    fn test_whitespace_required() {
        let parser = just("a")
            .then(whitespace_required())
            .then(just("b"))
            .then_ignore(end());
        assert!(parser.parse("a b").into_result().is_ok());
        assert!(parser.parse("a  b").into_result().is_ok());
        assert!(parser.parse("a\nb").into_result().is_ok());
        assert!(parser.parse("ab").into_result().is_err());
    }

    #[test]
    fn test_inline_whitespace() {
        let parser = inline_whitespace().then(just("test")).then_ignore(end());
        assert!(parser.parse("test").into_result().is_ok());
        assert!(parser.parse(" test").into_result().is_ok());
        assert!(parser.parse("\ttest").into_result().is_ok());
        // Should NOT consume newlines
        assert!(parser.parse("\ntest").into_result().is_err());
    }

    #[test]
    fn test_mermaid_comment() {
        let parser = mermaid_comment().then_ignore(end());
        assert!(parser.parse("%% this is a comment").into_result().is_ok());
        assert!(parser.parse("%%comment").into_result().is_ok());
        // Not a comment
        assert!(parser.parse("% not a comment").into_result().is_err());
    }
}
