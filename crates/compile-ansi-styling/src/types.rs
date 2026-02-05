//! Basic types.

use core::{fmt, ops};

use anstyle::Style;

use crate::{
    AnsiError,
    ansi_parser::AnsiParser,
    utils::{Stack, StackStr},
};

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
#[derive(Debug, Clone, Copy, Default)]
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

impl DynStyled {
    /// # Errors
    ///
    /// Returns an error if the input is not a valid ANSI escaped string.
    pub fn from_ansi(ansi_bytes: &[u8]) -> Result<Self, AnsiError> {
        AnsiParser::parse(ansi_bytes)
    }

    /// Unites sequential spans with the same color spec.
    pub(crate) fn shrink(self) -> Self {
        let mut shrunk_spans = Vec::<StyledSpan>::with_capacity(self.spans.len());
        for span in self.spans {
            if let Some(last_span) = shrunk_spans.last_mut() {
                if last_span.style == span.style {
                    last_span.len += span.len;
                } else {
                    shrunk_spans.push(span);
                }
            } else {
                shrunk_spans.push(span);
            }
        }

        Self {
            text: self.text,
            spans: shrunk_spans,
        }
    }
}

// FIXME: also implement fmt::Display outputting rich styles
impl<T, S> Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: ops::Deref<Target = [StyledSpan]>,
{
    /// Returns a string with embedded ANSI escapes.
    pub fn ansi(&self) -> impl fmt::Display + '_ {
        Ansi {
            text: &self.text,
            spans: &self.spans,
        }
    }
}

struct Ansi<'a> {
    text: &'a str,
    spans: &'a [StyledSpan],
}

impl fmt::Display for Ansi<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pos = 0;
        for span in self.spans {
            write!(
                formatter,
                "{style}{text}{style:#}",
                style = span.style,
                text = &self.text[pos..pos + span.len]
            )?;
            pos += span.len;
        }
        Ok(())
    }
}

impl<Tl, Sl, Tr, Sr> PartialEq<Styled<Tr, Sr>> for Styled<Tl, Sl>
where
    Tl: ops::Deref<Target = str>,
    Sl: ops::Deref<Target = [StyledSpan]>,
    Tr: ops::Deref<Target = str>,
    Sr: ops::Deref<Target = [StyledSpan]>,
{
    fn eq(&self, other: &Styled<Tr, Sr>) -> bool {
        *self.text == *other.text && *self.spans == *other.spans
    }
}

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
