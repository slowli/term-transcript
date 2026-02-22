//! `StyledString` and its builder.

use core::{fmt, mem, num::NonZeroUsize, ops};

use anstyle::Style;

use super::{StyledStr, slice::SpansSlice, spans::StyledSpan};
use crate::{
    AnsiError,
    alloc::{String, Vec},
    ansi_parser::AnsiParser,
    utils::normalize_style,
};

/// Builder for [`StyledString`]s.
///
/// A builder can be initialized from scratch via [`StyledString::builder()`] or [`Default`].
/// Alternatively, it can be initialized from an existing [`StyledString`].
/// A builder can be extended by pushing [text](Self::push_text()), [styles](Self::push_style())
/// or [`StyledStr`]ings into it.
///
/// # Examples
///
/// ```
/// # use styled_str::{styled, StyledString};
/// # use anstyle::{AnsiColor, Style};
/// let mut builder = StyledString::builder();
/// builder.push_style(AnsiColor::BrightGreen.on(AnsiColor::White).bold());
/// builder.push_text("Hello");
/// builder.push_text(",");
/// // It's possible to use `+=` as syntactic sugar for `push_str()` / `push_style()`.
/// builder += Style::new();
/// builder.push_text(" world");
/// builder += styled!("[[it, dim]]!");
///
/// let s = builder.build();
/// assert_eq!(
///     s.to_string(),
///     "[[bold green! on white]]Hello,[[/]] world[[italic dim]]!"
/// );
/// ```
#[derive(Debug, Clone, Default)]
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

impl From<StyledString> for StyledStringBuilder {
    fn from(string: StyledString) -> Self {
        let current_style = string
            .spans
            .last()
            .map_or_else(Style::new, |span| span.style);
        Self {
            inner: string,
            current_style,
        }
    }
}

impl ops::AddAssign<Style> for StyledStringBuilder {
    fn add_assign(&mut self, rhs: Style) {
        self.push_style(rhs);
    }
}

impl ops::AddAssign<StyledStr<'_>> for StyledStringBuilder {
    fn add_assign(&mut self, rhs: StyledStr<'_>) {
        self.push_str(rhs);
    }
}

// We don't implement `AddAssign<&str>` for `StyledStringBuilder` to avoid ambiguity what the string represents.

/// Heap-allocated styled string.
///
/// `StyledString` represents the owned string variant in contrast to [`StyledStr`], which is borrowed.
/// Since [conversion](Self::as_str()) to a `StyledStr` is cheap, some immutable methods are accessible via [`StyledStr`]
/// only (e.g., [iterating over style spans](StyledStr::spans()) or [splitting a string](StyledStr::split_at())).
/// This allows to define these methods as `const fn`s and/or to propagate the borrowed data lifetime correctly.
///
/// A `StyledString` can be parsed from [rich syntax](crate#rich-syntax) via the [`FromStr`](core::str::FromStr) trait,
/// from a string with ANSI escapes via [`StyledString::from_ansi()`], or manually constructed via [`StyledStringBuilder`].
#[derive(Clone, Default, PartialEq)]
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::{styled, StyledString};
    /// let str = StyledString::from_ansi(
    ///     "\u{1b}[1;32mHello,\u{1b}[m world\u{1b}[3m!\u{1b}[m",
    /// )?;
    /// assert_eq!(str, styled!("[[bold green]]Hello,[[/]] world[[it]]!"));
    /// # anyhow::Ok(())
    /// ```
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
    ///
    /// Use [`StyledStringBuilder`] for more complex string manipulations.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::{styled, StyledString};
    /// let mut styled = styled!("[[bold green!]]Hello").to_owned();
    /// styled.push_str(styled!("[[it]]!"));
    /// // `push_str()` is also available via `+=` operator:
    /// styled += styled!("[[it]]❤");
    ///
    /// assert_eq!(
    ///     styled,
    ///     styled!("[[bold green!]]Hello[[it]]!❤")
    /// );
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::{styled, StyledString};
    /// # use anstyle::Style;
    /// let mut styled = styled!("[[bold green!]]Hello[[it]]!❤").to_owned();
    /// let (ch, style) = styled.pop().unwrap();
    /// assert_eq!(ch, '❤');
    /// assert_eq!(style, Style::new().italic());
    /// assert_eq!(styled, styled!("[[bold green!]]Hello[[it]]!"));
    ///
    /// styled.pop().unwrap();
    /// assert_eq!(styled, styled!("[[bold green!]]Hello"));
    /// ```
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

impl ops::AddAssign<StyledStr<'_>> for StyledString {
    fn add_assign(&mut self, rhs: StyledStr<'_>) {
        self.push_str(rhs);
    }
}

impl<T> fmt::Debug for StyledString<T>
where
    T: ops::Deref<Target = str>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.as_str(), formatter)
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
