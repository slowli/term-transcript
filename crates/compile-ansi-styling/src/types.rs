//! Basic types.

use anstyle::Style;

use crate::utils::{Stack, StackStr};

/// Continuous span of styled text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyledSpan {
    /// Style applied to the text.
    pub style: Style,
    /// Length of text in bytes.
    pub len: usize,
}

/// ANSI-styled text.
///
/// FIXME: ways to create
#[derive(Debug, Clone, Copy)]
pub struct Styled<T = &'static str, S = &'static [StyledSpan]> {
    pub(crate) text: T,
    pub(crate) spans: S,
}

impl Styled {
    /// Gets the text without styling.
    pub const fn text(&self) -> &str {
        self.text
    }

    /// Gets the spans for the text.
    pub const fn spans(&self) -> &[StyledSpan] {
        self.spans
    }
}

/// Dynamic (i.e., non-compile time) variation of [`Styled`].
pub type DynStyled = Styled<String, Vec<StyledSpan>>;

/// Stack-allocated version of [`Styled`] for use in compile-time parsing of rich styling strings.
#[doc(hidden)]
#[derive(Debug)]
pub struct StackStyled<const TEXT_CAP: usize, const SPAN_CAP: usize> {
    pub(crate) text: StackStr<TEXT_CAP>,
    pub(crate) spans: Stack<StyledSpan, SPAN_CAP>,
}

impl<const TEXT_CAP: usize, const SPAN_CAP: usize> StackStyled<TEXT_CAP, SPAN_CAP> {
    /// Instantiates a new instance from a `rich`-flavored string.
    ///
    /// # Panics
    ///
    /// Panics if the rich syntax is invalid.
    #[track_caller]
    pub const fn new(raw: &str) -> Self {
        match Self::parse(raw) {
            Ok(styled) => styled,
            Err(err) => err.compile_panic(raw),
        }
    }

    pub const fn as_ref(&'static self) -> Styled {
        Styled {
            text: self.text.as_str(),
            spans: self.spans.as_slice(),
        }
    }
}
