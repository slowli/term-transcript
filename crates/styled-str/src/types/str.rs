//! `StyledStr`.

use core::{fmt, ops};

use anstyle::Style;

use super::{slice::SpansSlice, spans::StyledSpan};
use crate::{
    Diff, Lines, RichStyle, SpanStr, StyleDiff, StyledString, TextDiff, rich_parser::EscapedText,
};

/// ANSI-styled text.
///
/// A `Styled` instance consists of two parts:
///
/// - Original text (a `String` or a `&str`).
/// - A sequence of styled spans covering the text (`S` type param; usually a slice `&[StyledSpan]`
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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StyledStr<'a> {
    pub(crate) text: &'a str,
    pub(crate) spans: SpansSlice<'a>,
}

impl<'a> StyledStr<'a> {
    /// Checks whether this string is empty.
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Checks whether this string is plain (doesn't include non-default styled spans).
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::{StyledStr, styled};
    /// assert!(StyledStr::default().is_plain());
    /// assert!(styled!("Hello").is_plain());
    /// assert!(!styled!("[[green]]Hello").is_plain());
    /// ```
    // TODO: can be made const fn
    pub fn is_plain(&self) -> bool {
        if self.spans.len() > 1 {
            return false;
        }
        self.spans.get(0).is_none_or(|span| span.style.is_plain())
    }

    /// Returns the unstyled text.
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Diffs this against the `other` styled string.
    ///
    /// # Errors
    ///
    /// Returns an error if the styled strings differ either in text, or in applied styles.
    pub fn diff(self, other: Self) -> Result<(), Diff<'a>> {
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
    /// Panics in the same situations as [`crate::types::str::split_at()`].
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
        self.spans
            .iter()
            .map(|span| Self::map_span(self.text, span))
    }

    fn map_span(text: &'a str, span: StyledSpan) -> SpanStr<'a> {
        SpanStr {
            text: &text[span.start..span.end()],
            style: span.style,
        }
    }

    /// Returns a span by the *span* index.
    ///
    /// Use [`Self::span_at()`] if you need to locate the span covering the specified position in the text.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// # use anstyle::AnsiColor;
    /// let styled = styled!("[[bold blue!]]INFO[[/]] [[dim it]](2 min ago)[[/]] Important");
    /// let span = styled.span(0).unwrap();
    /// assert_eq!(span.text, "INFO");
    /// assert_eq!(span.style.get_fg_color(), Some(AnsiColor::BrightBlue.into()));
    ///
    /// let span = styled.span(2).unwrap();
    /// assert_eq!(span.text, "(2 min ago)");
    /// ```
    // TODO: can be made const fn
    pub fn span(&self, span_idx: usize) -> Option<SpanStr<'a>> {
        let span = self.spans.get(span_idx)?;
        Some(Self::map_span(self.text, span))
    }

    /// Looks up a span covering the specified position in the unstyled text.
    ///
    /// Use [`Self::span()`] if you need to locate the span by its index.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// # use anstyle::Effects;
    /// let styled = styled!("[[bold blue!]]INFO[[/]] [[dim it]](2 min ago)[[/]] Important");
    /// let span = styled.span_at(7).unwrap();
    /// assert_eq!(span.text, "(2 min ago)");
    /// assert_eq!(span.style.get_effects(), Effects::ITALIC | Effects::DIMMED);
    /// ```
    // TODO: can be made const fn
    pub fn span_at(&self, text_pos: usize) -> Option<SpanStr<'a>> {
        if text_pos >= self.text.len() {
            return None;
        }
        let span = self.spans.get_by_text_pos(text_pos)?;
        Some(Self::map_span(self.text, span))
    }

    /// Splits this text by lines.
    pub fn lines(self) -> Lines<'a> {
        Lines::new(self)
    }

    /// Returns a string with embedded ANSI escapes.
    pub fn ansi(&self) -> impl fmt::Display + '_ {
        Ansi(*self)
    }

    /// Pops a single char from the end of the string.
    pub fn pop(&mut self) -> Option<(char, Style)> {
        let ch = self.text.chars().next_back()?;
        self.text = &self.text[..self.text.len() - ch.len_utf8()];
        let style = self.spans.pop_char(ch.len_utf8());
        Some((ch, style))
    }
}

impl<'a, T> From<StyledStr<'a>> for StyledString<T>
where
    T: From<&'a str> + ops::Deref<Target = str>,
{
    fn from(str: StyledStr<'a>) -> Self {
        Self {
            text: str.text.into(),
            spans: str.spans.iter().collect(),
        }
    }
}

#[derive(Debug)]
struct Ansi<'a>(StyledStr<'a>);

impl fmt::Display for Ansi<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for span in self.0.spans.iter() {
            write!(
                formatter,
                "{style}{text}{style:#}",
                style = span.style,
                text = &self.0.text[span.start..span.end()]
            )?;
        }
        Ok(())
    }
}

impl<T> PartialEq<StyledString<T>> for StyledStr<'_>
where
    T: ops::Deref<Target = str>,
{
    fn eq(&self, other: &StyledString<T>) -> bool {
        *self == other.as_str()
    }
}

impl<T> PartialEq<StyledStr<'_>> for StyledString<T>
where
    T: ops::Deref<Target = str>,
{
    fn eq(&self, other: &StyledStr<'_>) -> bool {
        self.as_str() == *other
    }
}

/// Outputs a string with rich syntax.
impl fmt::Display for StyledStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, span) in self.spans.iter().enumerate() {
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
