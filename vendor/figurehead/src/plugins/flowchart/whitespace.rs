//! Shared whitespace/comment helpers for flowchart parsing.

use chumsky::prelude::*;
use chumsky::text::whitespace;

/// Parsers whitespace and Mermaid comment segments.
pub fn whitespace_segment<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    let comment = just("%%").ignore_then(none_of('\n').repeated()).ignored();

    whitespace().or(comment).ignored()
}

/// Optional whitespace/comment parser.
pub fn optional_whitespace<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
    whitespace_segment().or_not().ignored()
}
