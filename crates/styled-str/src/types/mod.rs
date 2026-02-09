//! Basic types.

use core::{fmt, num::NonZeroUsize, ops};

use anstyle::Style;

pub use self::{lines::Lines, slice::SpansSlice, traits::PopChar};
use crate::{
    AnsiError, StyleDiff,
    alloc::{String, Vec},
    ansi_parser::AnsiParser,
    rich_parser::{EscapedText, RichStyle},
    utils::{Stack, StackStr, normalize_style},
};

mod lines;
mod slice;
mod traits;

/// Continuous span of styled text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyledSpan {
    /// Style applied to the text.
    pub style: Style,
    /// Length of text in bytes.
    pub len: NonZeroUsize,
}

impl StyledSpan {
    /// # Panics
    ///
    /// Panics if `len` is zero.
    pub const fn new(style: Style, len: usize) -> Self {
        Self {
            style,
            len: NonZeroUsize::new(len).unwrap(),
        }
    }

    #[doc(hidden)] // low-level
    pub const fn extend_len(&mut self, add: usize) {
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
pub type StyledStr<'a> = Styled<&'a str, &'a [StyledSpan]>;

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

    /// Splits this text by lines.
    pub fn lines(self) -> Lines<'a> {
        Lines::new(self)
    }
}

/// Dynamic (i.e., non-compile time) variation of [`Styled`].
pub type StyledString = Styled<String, Vec<StyledSpan>>;

impl StyledString {
    /// Empty string.
    pub const EMPTY: Self = Self {
        text: String::new(),
        spans: Vec::new(),
    };

    /// Creates a string from the provided text and spans. This will normalize styles and merge
    /// sequential spans with the same style if appropriate.
    ///
    /// # Panics
    ///
    /// Panics if `text` and `spans` have differing lengths.
    pub fn from_parts(text: String, mut spans: Vec<StyledSpan>) -> Self {
        assert!(
            text.bytes().all(|ch| ch != 0x1b),
            "Text contains 0x1b escape char"
        );
        assert_eq!(
            spans.iter().map(|span| span.len.get()).sum::<usize>(),
            text.len(),
            "Mismatch between total length of spans and text length"
        );

        let mut pos = 0;
        for (i, span) in spans.iter().enumerate() {
            assert!(
                text.is_char_boundary(pos),
                "span #{i} does not start at char boundary"
            );
            pos += span.len.get();
        }

        for span in &mut spans {
            span.style = normalize_style(span.style);
        }
        Self { text, spans }.shrink()
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
        self.text.push_str(other.text);

        let mut copied_spans = other.spans;
        if let (Some(last), Some(next)) = (self.spans.last_mut(), other.spans.first()) {
            if last.style == next.style {
                last.extend_len(next.len.get());
                copied_spans = &other.spans[1..];
            }
        }
        self.spans.extend_from_slice(copied_spans);
    }

    /// Unites sequential spans with the same color spec.
    pub(crate) fn shrink(self) -> Self {
        let mut shrunk_spans = Vec::<StyledSpan>::with_capacity(self.spans.len());
        for span in self.spans {
            if let Some(last_span) = shrunk_spans.last_mut() {
                if last_span.style == span.style {
                    last_span.extend_len(span.len.get());
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

impl<T, S> Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: ops::Deref<Target = [StyledSpan]>,
{
    /// Returns the unstyled text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the style spans in this string.
    pub fn spans(&self) -> &[StyledSpan] {
        &self.spans
    }

    /// Returns a borrowed version of this string.
    pub fn as_ref(&self) -> StyledStr<'_> {
        Styled {
            text: self.text(),
            spans: self.spans(),
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
        Sr: ops::Deref<Target = [StyledSpan]>,
    {
        self.as_ref().diff_inner(other.as_ref())
    }

    /// Returns a string with embedded ANSI escapes.
    pub fn ansi(&self) -> impl fmt::Display + '_ {
        Ansi {
            text: &self.text,
            spans: &self.spans,
        }
    }
}

impl From<StyledStr<'_>> for StyledString {
    fn from(str: StyledStr<'_>) -> Self {
        Self {
            text: (*str.text).into(),
            spans: str.spans.to_vec(),
        }
    }
}

impl<T, S> FromIterator<Styled<T, S>> for StyledString
where
    T: ops::Deref<Target = str>,
    S: ops::Deref<Target = [StyledSpan]>,
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
    S: ops::Deref<Target = [StyledSpan]>,
{
    fn extend<I: IntoIterator<Item = Styled<T, S>>>(&mut self, iter: I) {
        for str in iter {
            self.push_str(str.as_ref());
        }
    }
}

impl<T> Styled<T, Vec<StyledSpan>>
where
    T: ops::Deref<Target = str> + PopChar,
{
    /// Pops a single char from the end of the string.
    #[allow(clippy::missing_panics_doc)] // internal errors; should never be triggered
    pub fn pop(&mut self) -> Option<(char, Style)> {
        let ch = self.text.pop_char()?;
        let ch_len = ch.len_utf8();

        let last_span = self
            .spans
            .last_mut()
            .expect("internal error: text is empty, but spans aren't");
        assert!(last_span.len.get() >= ch_len, "style span divides char");
        let style = last_span.style;
        if let Some(new_len) = NonZeroUsize::new(last_span.len.get() - ch_len) {
            last_span.len = new_len;
        } else {
            self.spans.pop();
        }
        Some((ch, style))
    }
}

/// Outputs a string with rich syntax.
impl<T, S> fmt::Display for Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: ops::Deref<Target = [StyledSpan]>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pos = 0;
        for (i, span) in self.spans.iter().enumerate() {
            let text = &self.text[pos..pos + span.len.get()];
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
            pos += span.len.get();
        }
        Ok(())
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
                text = &self.text[pos..pos + span.len.get()]
            )?;
            pos += span.len.get();
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
            spans: self.spans.as_slice(),
        }
    }
}
