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

impl std::error::Error for HexColorError {}

/// Kind of a [`ParseError`].
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// Unfinished style, e.g. in `[[red`.
    UnfinishedStyle,
    /// Unsupported token in style, e.g. `[[what]]`.
    UnsupportedStyle,
    /// Error parsing a hexadecimal color spec, e.g. in `#c0g`.
    HexColor(HexColorError),
    /// Unfinished color specification, e.g. `color(`.
    UnfinishedColor,
    /// Invalid index color, e.g. `1234`.
    InvalidIndexColor,
    /// `on` token without the following color.
    UnfinishedBackground,
    /// Bogus delimiter encountered, e.g. in `[[red] on white]]`.
    BogusDelimiter,
    /// `*` token (copying previously used style) must be the first token in the spec.
    NonInitialCopy,
    /// `/` token (clearing style) must be the only token in the spec.
    NonIsolatedClear,
    /// Unsupported effect in a negation, e.g. `[[* -red]]`.
    UnsupportedEffect,
    /// Negation
    NegationWithoutCopy,
    /// Duplicate specified for the same property, like `[[bold bold]]` or `[[red green]]`.
    DuplicateSpecifier,
    /// Redundant negation, e.g. in `[[* -bold -bold]]`.
    RedundantNegation,
    /// ANSI escape char `\u{1b}` encountered in the text.
    EscapeInText,

    #[doc(hidden)] // should not occur unless private APIs are used
    SpanOverflow,
    #[doc(hidden)] // should not occur unless private APIs are used
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
            Self::UnfinishedColor => "unfinished color spec",
            Self::InvalidIndexColor => "invalid indexed color",
            Self::UnfinishedBackground => "no background specified after `on` keyword",
            Self::BogusDelimiter => "bogus delimiter",
            Self::NonInitialCopy => "* (copy) specifier must come first",
            Self::NonIsolatedClear => "/ (clear) specifier must be the only token",
            Self::UnsupportedEffect => "unsupported effect",
            Self::NegationWithoutCopy => "negation without * (copy) specifier",
            Self::DuplicateSpecifier => "duplicate specifier",
            Self::RedundantNegation => "redundant negation",
            Self::EscapeInText => "ANSI escape char 0x1b encountered in text",
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

/// Errors that can occur parsing [`Styled`](crate::Styled) strings from the [rich syntax](crate#rich-syntax).
#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
    pos: ops::Range<usize>,
}

impl ParseError {
    /// Returns the kind of this error.
    pub const fn kind(&self) -> &ParseErrorKind {
        &self.kind
    }

    /// Returns (byte) position in the source string that corresponds to this error.
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
            "'): ", self.kind.as_ascii_str() => compile_fmt::clip_ascii(42, "")
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
