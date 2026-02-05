//! Error types.

use core::{ops, str};

use compile_fmt::{Ascii, compile_panic};

#[derive(Debug)]
#[non_exhaustive]
pub enum ParseErrorKind {
    UnfinishedStyle,
    UnsupportedStyle,
    InvalidHexColor,
    InvalidIndexColor,
    RedefinedBackground,
    UnfinishedBackground,
    BogusDelimiter,
    SpanOverflow,
    TextOverflow,
}

impl ParseErrorKind {
    pub(crate) const fn with_pos(self, pos: ops::Range<usize>) -> ParseError {
        ParseError { kind: self, pos }
    }

    const fn as_str(&self) -> &'static str {
        match self {
            Self::UnfinishedStyle => "unfinished style definition",
            Self::UnsupportedStyle => "unsupported style specifier",
            Self::InvalidHexColor => "invalid hex color definition",
            Self::InvalidIndexColor => "invalid indexed color",
            Self::UnfinishedBackground => "no background specified after `on` keyword",
            Self::RedefinedBackground => "redefined background color",
            Self::BogusDelimiter => "bogus delimiter",
            Self::SpanOverflow => "too many spans",
            Self::TextOverflow => "too much text",
        }
    }

    const fn as_ascii_str(&self) -> Ascii<'static> {
        Ascii::new(self.as_str())
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

    #[track_caller]
    pub(crate) const fn compile_panic(self, raw: &str) -> ! {
        let (_, hl) = raw.as_bytes().split_at(self.pos.start);
        let (hl, _) = hl.split_at(self.pos.end - self.pos.start);
        let Ok(hl) = str::from_utf8(hl) else {
            panic!("internal error: invalid error range");
        };

        compile_panic!(
            "invalid regex at ",
            self.pos.start => compile_fmt::fmt::<usize>(), "..", self.pos.end => compile_fmt::fmt::<usize>(),
            " ('", hl => compile_fmt::clip(64, "…"),
            "'): ", self.kind.as_ascii_str() => compile_fmt::clip_ascii(32, "")
        );
    }
}
