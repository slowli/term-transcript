//! Error types.

use core::{fmt, ops, str};

use compile_fmt::{Ascii, compile_panic};

/// Error parsing hexadecimal RGB color.
#[derive(Debug)]
#[non_exhaustive]
pub enum HexColorError {
    /// Color string doesn't start with a hash `#`.
    NoHash,
    /// Color string has unexpected length (not 4 or 7).
    InvalidLen,
    /// Color string contains an invalid hex digit.
    InvalidHexDigit,
}

impl fmt::Display for HexColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl HexColorError {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::NoHash => "color string doesn't start with a hash `#`",
            Self::InvalidLen => "color string has unexpected length (not 4 or 7)",
            Self::InvalidHexDigit => "color string contains an invalid hex digit",
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ParseErrorKind {
    UnfinishedStyle,
    UnsupportedStyle,
    HexColor(HexColorError),
    InvalidIndexColor,
    RedefinedBackground,
    UnfinishedBackground,
    BogusDelimiter,
    NonInitialCopy,
    UnsupportedEffect,
    NegationWithoutCopy,
    DuplicateSpecifier,
    RedundantNegation,
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
            Self::HexColor(err) => err.as_str(),
            Self::InvalidIndexColor => "invalid indexed color",
            Self::UnfinishedBackground => "no background specified after `on` keyword",
            Self::RedefinedBackground => "redefined background color",
            Self::BogusDelimiter => "bogus delimiter",
            Self::NonInitialCopy => "* (copy) specifier must come first",
            Self::UnsupportedEffect => "unsupported effect",
            Self::NegationWithoutCopy => "negation without * (copy) specifier",
            Self::DuplicateSpecifier => "duplicate specifier",
            Self::RedundantNegation => "redundant negation",
            Self::SpanOverflow => "too many spans",
            Self::TextOverflow => "too much text",
        }
    }

    const fn as_ascii_str(&self) -> Ascii<'static> {
        Ascii::new(self.as_str())
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
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
            "invalid styled string at ",
            self.pos.start => compile_fmt::fmt::<usize>(), "..", self.pos.end => compile_fmt::fmt::<usize>(),
            " ('", hl => compile_fmt::clip(64, "…"),
            "'): ", self.kind.as_ascii_str() => compile_fmt::clip_ascii(40, "")
        );
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid styled string at {:?}: {}",
            self.pos, self.kind
        )
    }
}

impl std::error::Error for ParseError {}
