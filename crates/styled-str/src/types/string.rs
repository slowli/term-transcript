//! `StyledString` and its builder.

use core::{fmt, mem, num::NonZeroUsize, ops};

use anstyle::Style;

use super::{StyledStr, slice::SpansSlice, spans::StyledSpan};
use crate::{AnsiError, ansi_parser::AnsiParser, utils::normalize_style};

/// Builder for [`StyledString`]s.
#[derive(Debug, Default)]
pub struct StyledStringBuilder {
    inner: StyledString,
    current_style: Style,
}

impl StyledStringBuilder {
    /// Returns the current string text.
    pub fn text(&self) -> &str {
        self.inner.text()
    }

    /// Pushes unstyled text at the end of the string.
    ///
    /// # Panics
    ///
    /// Panics if `text` contains an ANSI escape char (`\x1b`).
    pub fn push_text(&mut self, text: &str) {
        assert!(
            text.bytes().all(|ch| ch != 0x1b),
            "Text contains 0x1b escape char"
        );
        self.inner.text.push_str(text);
    }

    pub(crate) fn current_style(&self) -> &Style {
        &self.current_style
    }

    /// Pushes a style at the end of this string.
    pub fn push_style(&mut self, style: Style) {
        let style = normalize_style(style);
        let spanned_text_len = self.inner.spans.last().map_or(0, StyledSpan::end);
        let prev_style = mem::replace(&mut self.current_style, style);
        if let Some(len) = NonZeroUsize::new(self.inner.text.len() - spanned_text_len) {
            self.push_span(StyledSpan {
                style: prev_style,
                start: spanned_text_len,
                len,
            });
        }
    }

    fn push_span(&mut self, span: StyledSpan) {
        if let Some(last_span) = self.inner.spans.last_mut() {
            if last_span.style == span.style {
                last_span.extend_len(span.len.get());
                return;
            }
        }
        self.inner.spans.push(span);
    }

    /// Pushes a styled string at the end of this string.
    pub fn push_str(&mut self, s: StyledStr<'_>) {
        // Flush the current style so that `self.inner` is well-formed, which `StyledString::push_str()` relies upon.
        self.push_style(self.current_style);
        self.inner.push_str(s);

        if let Some(last_span) = self.inner.spans.last() {
            self.current_style = last_span.style;
        }
    }

    /// Finalizes the [`StyledString`].
    pub fn build(mut self) -> StyledString {
        // Push the last style span covering the non-spanned text
        self.push_style(Style::new());
        self.inner
    }
}

/// Heap-allocated styled string.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyledString<T = String> {
    pub(crate) text: T,
    pub(crate) spans: Vec<StyledSpan>,
}

impl<T> StyledString<T>
where
    T: ops::Deref<Target = str>,
{
    /// Returns the unstyled text behind this string.
    pub fn into_text(self) -> T {
        self.text
    }

    /// Borrows a [`StyledStr`] from this string. This can be used to call more complex methods.
    pub fn as_str(&self) -> StyledStr<'_> {
        StyledStr {
            text: &self.text,
            spans: SpansSlice::new(&self.spans),
        }
    }

    /// Returns the unstyled text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl StyledString {
    /// Creates a builder for styled strings.
    pub fn builder() -> StyledStringBuilder {
        StyledStringBuilder::default()
    }

    /// Parses a string from a string with embedded ANSI escape sequences.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid ANSI escaped string.
    pub fn from_ansi(ansi_str: &str) -> Result<Self, AnsiError> {
        AnsiParser::parse(ansi_str.as_bytes())
    }

    /// Parses a string from bytes with embedded ANSI escape sequences. This is similar to
    /// [`Self::from_ansi()`], just using bytes as an input.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid ANSI escaped string.
    pub fn from_ansi_bytes(ansi_bytes: &[u8]) -> Result<Self, AnsiError> {
        AnsiParser::parse(ansi_bytes)
    }

    /// Pushes another styled string at the end of this one.
    pub fn push_str(&mut self, other: StyledStr<'_>) {
        let mut copied_spans = other.spans.iter();
        if let (Some(last), Some(next)) = (self.spans.last_mut(), other.spans.get(0)) {
            if last.style == next.style {
                last.extend_len(next.len.get());
                copied_spans.next(); // skip copying the first span
            }
        }

        // We need to offset the newly added spans, so that their start positions are correct.
        let offset = self.text.len();
        self.spans.extend(copied_spans.map(|mut span| {
            span.start += offset;
            span
        }));

        self.text.push_str(other.text);
    }

    /// Pops a single char from the end of the string.
    #[allow(clippy::missing_panics_doc)] // internal checks; should never be triggered
    pub fn pop(&mut self) -> Option<(char, Style)> {
        let ch = self.text.pop()?;
        let char_len = ch.len_utf8();

        let last_span = self.spans.last_mut().unwrap();
        assert!(last_span.len.get() >= char_len, "style span divides char");

        let style = last_span.style;
        if let Some(new_len) = NonZeroUsize::new(last_span.len.get() - char_len) {
            last_span.len = new_len;
        } else {
            self.spans.pop();
        }
        Some((ch, style))
    }
}

impl<T> fmt::Display for StyledString<T>
where
    T: ops::Deref<Target = str>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.as_str(), formatter)
    }
}

impl<'a> FromIterator<StyledStr<'a>> for StyledString {
    fn from_iter<I: IntoIterator<Item = StyledStr<'a>>>(iter: I) -> Self {
        iter.into_iter()
            .fold(StyledString::default(), |mut acc, str| {
                acc.push_str(str);
                acc
            })
    }
}

impl<'a> Extend<StyledStr<'a>> for StyledString {
    fn extend<I: IntoIterator<Item = StyledStr<'a>>>(&mut self, iter: I) {
        for str in iter {
            self.push_str(str);
        }
    }
}
