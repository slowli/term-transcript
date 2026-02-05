//! Error types.

use core::ops;

#[derive(Debug)]
#[non_exhaustive]
pub enum ParseErrorKind {
    UnfinishedStyle,
    UnsupportedStyle,
    InvalidHexColor,
    InvalidIndexColor,
    RedefinedBackground,
    UnfinishedBackground,
    SpanOverflow,
    TextOverflow,
}

impl ParseErrorKind {
    pub(crate) const fn with_pos(self, pos: ops::Range<usize>) -> ParseError {
        ParseError { kind: self, pos }
    }
}

#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
    pos: ops::Range<usize>,
}

impl ParseError {
    pub const fn kind(&self) -> &ParseErrorKind {
        &self.kind
    }

    pub fn pos(&self) -> ops::Range<usize> {
        self.pos.clone()
    }
}
