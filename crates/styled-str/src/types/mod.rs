//! Basic types.

use core::{fmt, mem, num::NonZeroUsize, ops};

use anstyle::Style;
use compile_fmt::compile_panic;

pub use self::{
    lines::Lines,
    slice::{AsSpansSlice, SpansSlice, SpansVec},
    traits::PopChar,
};
use crate::{
    AnsiError, StyleDiff,
    alloc::String,
    ansi_parser::AnsiParser,
    rich_parser::{EscapedText, RichStyle},
    utils::{Stack, StackStr, normalize_style},
};

mod lines;
mod slice;
mod traits;

/// Continuous span of styled text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StyledSpan {
    /// Style applied to the text.
    pub(crate) style: Style,
    /// Starting position of the span in text.
    pub(crate) start: usize,
    /// Length of text in bytes.
    pub(crate) len: NonZeroUsize,
}

impl StyledSpan {
    pub(crate) const DUMMY: Self = Self {
        style: Style::new(),
        start: 0,
        len: NonZeroUsize::new(1).unwrap(),
    };

    pub(crate) const fn end(&self) -> usize {
        self.start + self.len.get()
    }

    pub(crate) const fn extend_len(&mut self, add: usize) {
        self.len = self.len.checked_add(add).expect("length overflow");
    }

    pub(crate) fn shrink_len(&mut self, sub: usize) {
        self.len = self
            .len
            .get()
            .checked_sub(sub)
            .and_then(NonZeroUsize::new)
            .expect("length underflow");
    }
}

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
        let spanned_text_len = self.inner.spans.0.last().map_or(0, StyledSpan::end);
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
        if let Some(last_span) = self.inner.spans.0.last_mut() {
            if last_span.style == span.style {
                last_span.extend_len(span.len.get());
                return;
            }
        }
        self.inner.spans.0.push(span);
    }

    /// Pushes a styled string at the end of this string.
    pub fn push_str(&mut self, s: StyledStr<'_>) {
        // Flush the current style so that `self.inner` is well-formed, which `StyledString::push_str()` relies upon.
        self.push_style(self.current_style);
        self.inner.push_str(s);

        if let Some(last_span) = self.inner.spans.0.last() {
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

/// Text with a uniform [`Style`] attached to it. Returned by the [`StyledStr::spans()`] iterator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpanStr<'a> {
    /// Unstyled text.
    pub text: &'a str,
    /// Style applied to the text.
    pub style: Style,
}

impl<'a> SpanStr<'a> {
    /// Creates a string spanned with the specified style.
    ///
    /// # Panics
    ///
    /// Panics if `text` contains `\x1b` escapes.
    pub const fn new(text: &'a str, style: Style) -> Self {
        let text_bytes = text.as_bytes();
        let mut pos = 0;
        while pos < text_bytes.len() {
            if text_bytes[pos] == 0x1b {
                compile_panic!(
                    "text contains \\x1b escape, first at position ",
                    pos => compile_fmt::fmt::<usize>()
                );
            }
            pos += 1;
        }
        Self { text, style }
    }
}

/// ANSI-styled text.
///
/// A `Styled` instance consists of two parts:
///
/// - Original text (`T` type param; usually a `String` or a `&str`).
/// - A sequence of [`StyledSpan`]s covering the text (`S` type param; usually a slice `&[StyledSpan]`
///   or a `Vec<StyledSpan>`).
///
/// [`StyledStr`] and [`StyledString`] represent the borrowed / owned instantiations of the type, respectively.
///
/// - [`StyledStr`] can be parsed from [rich syntax](crate#rich-syntax) in compile time via the [`styled!`](crate::styled!) macro,
///   or borrowed from a string using [`Styled::as_ref()`].
///-  [`StyledString`] can be parsed from [rich syntax](crate#rich-syntax) via the [`FromStr`](core::str::FromStr) trait,
///   from a string with ANSI escapes via [`StyledString::from_ansi()`], or manually constructed via [`StyledString::from_parts()`].
///
/// # Examples
///
/// See [crate-level docs](crate) for the examples of usage.
#[derive(Debug, Clone, Copy, Default)]
pub struct Styled<T, S> {
    pub(crate) text: T,
    pub(crate) spans: S,
}

/// Borrowed version of [`Styled`].
pub type StyledStr<'a> = Styled<&'a str, SpansSlice<'a>>;

impl<'a> StyledStr<'a> {
    fn diff_inner(self, other: Self) -> Result<(), Diff<'a>> {
        if self.text == other.text {
            let style_diff = StyleDiff::new(self, other);
            if style_diff.is_empty() {
                Ok(())
            } else {
                Err(Diff::Style(style_diff))
            }
        } else {
            Err(Diff::Text(TextDiff::new(self.text, other.text)))
        }
    }

    /// Splits this string into two at the specified position.
    ///
    /// # Panics
    ///
    /// Panics in the same situations as [`str::split_at()`].
    pub fn split_at(self, mid: usize) -> (Self, Self) {
        let (start_text, end_text) = self.text.split_at(mid);
        let (start_spans, end_spans) = self.spans.split_at(mid);
        let start = Self {
            text: start_text,
            spans: start_spans,
        };
        let end = Self {
            text: end_text,
            spans: end_spans,
        };
        (start, end)
    }

    /// Iterates over spans contained in this string.
    pub fn spans(&self) -> impl ExactSizeIterator<Item = SpanStr<'a>> + DoubleEndedIterator + 'a {
        self.spans.iter().map(|span| SpanStr {
            text: &self.text[span.start..span.end()],
            style: span.style,
        })
    }

    /// Splits this text by lines.
    pub fn lines(self) -> Lines<'a> {
        Lines::new(self)
    }
}

/// Dynamic (i.e., non-compile time) variation of [`Styled`].
pub type StyledString = Styled<String, SpansVec>;

impl StyledString {
    /// Empty string.
    pub const EMPTY: Self = Self {
        text: String::new(),
        spans: SpansVec::EMPTY,
    };

    /// Creates a builder for
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
        if let (Some(last), Some(next)) = (self.spans.0.last_mut(), other.spans.get(0)) {
            if last.style == next.style {
                last.extend_len(next.len.get());
                copied_spans.next(); // skip copying the first span
            }
        }

        // We need to offset the newly added spans, so that their start positions are correct.
        let offset = self.text.len();
        self.spans.0.extend(copied_spans.map(|mut span| {
            span.start += offset;
            span
        }));

        self.text.push_str(other.text);
    }
}

impl<T, S> Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: AsSpansSlice,
{
    /// Returns the unstyled text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Checks whether this string is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns a borrowed version of this string.
    pub fn as_ref(&self) -> StyledStr<'_> {
        Styled {
            text: self.text(),
            spans: self.spans.as_slice(),
        }
    }

    /// Splits this string into parts (the text and [`StyledSpan`]s).
    pub fn into_parts(self) -> (T, S) {
        (self.text, self.spans)
    }

    /// Diffs this against the `other` styled string.
    ///
    /// # Errors
    ///
    /// Returns an error if the styled strings differ either in text, or in applied styles.
    pub fn diff<'s, Tr, Sr>(&'s self, other: &'s Styled<Tr, Sr>) -> Result<(), Diff<'s>>
    where
        Tr: ops::Deref<Target = str>,
        Sr: AsSpansSlice,
    {
        self.as_ref().diff_inner(other.as_ref())
    }

    /// Returns a string with embedded ANSI escapes.
    pub fn ansi(&self) -> impl fmt::Display + '_ {
        Ansi {
            text: &self.text,
            spans: self.spans.as_slice(),
        }
    }
}

impl From<StyledStr<'_>> for StyledString {
    fn from(str: StyledStr<'_>) -> Self {
        Self {
            text: (*str.text).into(),
            spans: SpansVec(str.spans.iter().collect()),
        }
    }
}

impl<T, S> FromIterator<Styled<T, S>> for StyledString
where
    T: ops::Deref<Target = str>,
    S: AsSpansSlice,
{
    fn from_iter<I: IntoIterator<Item = Styled<T, S>>>(iter: I) -> Self {
        iter.into_iter()
            .fold(StyledString::default(), |mut acc, str| {
                acc.push_str(str.as_ref());
                acc
            })
    }
}

impl<T, S> Extend<Styled<T, S>> for StyledString
where
    T: ops::Deref<Target = str>,
    S: AsSpansSlice,
{
    fn extend<I: IntoIterator<Item = Styled<T, S>>>(&mut self, iter: I) {
        for str in iter {
            self.push_str(str.as_ref());
        }
    }
}

impl<T, S> Styled<T, S>
where
    T: ops::Deref<Target = str> + PopChar,
    S: AsSpansSlice,
{
    /// Pops a single char from the end of the string.
    pub fn pop(&mut self) -> Option<(char, Style)> {
        let ch = self.text.pop_char()?;
        let style = self.spans.pop_char(ch.len_utf8());
        Some((ch, style))
    }
}

/// Outputs a string with rich syntax.
impl<T, S> fmt::Display for Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: AsSpansSlice,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, span) in self.spans.as_slice().iter().enumerate() {
            let text = &self.text[span.start..span.end()];
            if i == 0 && span.style.is_plain() {
                // Special case: do not output an extra `[[/]]` at the string start.
                write!(formatter, "{}", EscapedText(text))?;
            } else {
                write!(
                    formatter,
                    "[[{style}]]{text}",
                    style = RichStyle(&span.style),
                    text = EscapedText(text)
                )?;
            }
        }
        Ok(())
    }
}

struct Ansi<'a> {
    text: &'a str,
    spans: SpansSlice<'a>,
}

impl fmt::Display for Ansi<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for span in self.spans.iter() {
            write!(
                formatter,
                "{style}{text}{style:#}",
                style = span.style,
                text = &self.text[span.start..span.end()]
            )?;
        }
        Ok(())
    }
}

impl<Tl, Sl, Tr, Sr> PartialEq<Styled<Tr, Sr>> for Styled<Tl, Sl>
where
    Tl: ops::Deref<Target = str>,
    Sl: AsSpansSlice,
    Tr: ops::Deref<Target = str>,
    Sr: AsSpansSlice,
{
    fn eq(&self, other: &Styled<Tr, Sr>) -> bool {
        *self.text == *other.text && self.spans.as_slice() == other.spans.as_slice()
    }
}

/// Text difference between two strings. ANSI-styled when printed (powered by [`pretty_assertions::Comparison`]).
///
/// # [`Display`](fmt::Display) representation
///
/// You can specify additional padding at the start of compared lines
/// via alignment specifiers. For example, `{:>4}` will insert 4 spaces at the start of each line.
///
/// # Examples
///
/// ```
/// use styled_str::{StyledString, TextDiff};
///
/// let diff = TextDiff::new("Hello, world", "Hello world!");
/// let diff_str = StyledString::from_ansi(&format!("{diff:>4}"))?;
/// assert_eq!(
///     diff_str.text().trim(),
///     "Diff < left / right > :\n\
///      <   Hello, world\n\
///      >   Hello world!"
/// );
/// assert!(!diff_str.spans().is_empty());
/// # anyhow::Ok(())
/// ```
#[derive(Debug)]
pub struct TextDiff<'a> {
    lhs: &'a str,
    rhs: &'a str,
}

impl<'a> TextDiff<'a> {
    /// Computes difference between two strings.
    pub const fn new(lhs: &'a str, rhs: &'a str) -> Self {
        Self { lhs, rhs }
    }
}

impl fmt::Display for TextDiff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use pretty_assertions::Comparison;

        // Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
        // to switch to `fmt::Display`.
        struct DebugStr<'a> {
            s: &'a str,
            padding: usize,
        }

        impl<'a> DebugStr<'a> {
            fn new(s: &'a str, padding: usize) -> Self {
                Self { s, padding }
            }
        }

        impl fmt::Debug for DebugStr<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.padding == 0 {
                    formatter.write_str(self.s)
                } else {
                    for line in self.s.lines() {
                        writeln!(formatter, "{:>padding$}{line}", "", padding = self.padding)?;
                    }
                    Ok(())
                }
            }
        }

        let padding = if matches!(formatter.align(), Some(fmt::Alignment::Right) | None) {
            formatter.width().map_or(0, |width| width.saturating_sub(1))
        } else {
            0
        };

        write!(
            formatter,
            "{}",
            Comparison::new(
                &DebugStr::new(self.lhs, padding),
                &DebugStr::new(self.rhs, padding)
            )
        )
    }
}

/// Generic difference between two [`Styled`] strings: either a difference in text, or in styling.
///
/// Produced by the [`Styled::diff()`] method.
pub enum Diff<'a> {
    /// There is a difference in text between the compared strings.
    Text(TextDiff<'a>),
    /// String texts match, but there is a difference in ANSI styles.
    Style(StyleDiff<'a>),
}

impl fmt::Display for Diff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(diff) => write!(formatter, "styled strings differ by text\n{diff}"),
            Self::Style(diff) => write!(
                formatter,
                "styled strings differ by style\n{diff}\n{diff:#}"
            ),
        }
    }
}

// Delegates to `Display` to get better panic messages on `.diff(_).unwrap()`.
impl fmt::Debug for Diff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Diff<'_> {}

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

    pub const fn as_ref(&'static self) -> StyledStr<'static> {
        Styled {
            text: self.text.as_str(),
            spans: SpansSlice::new(self.spans.as_slice()),
        }
    }
}
