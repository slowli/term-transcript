//! `StyledStr`.

use core::{fmt, ops};

use anstyle::Style;

use super::{slice::SpansSlice, spans::StyledSpan};
use crate::{
    Diff, Lines, RichStyle, SpanStr, StyleDiff, StyledString, TextDiff, rich_parser::EscapedText,
    utils,
};

/// Borrowed ANSI-styled string.
///
/// A `StyledStr` instance consists of two parts:
///
/// - Original text (a `&str`).
/// - A sequence of styled spans covering the text.
///
/// `StyledStr` represents the borrowed string variant in contrast to [`StyledString`], which is owned.
/// A `StyledStr` can be parsed from [rich syntax](crate#rich-syntax) in compile time via the [`styled!`](crate::styled!) macro,
/// or borrowed from a string using [`StyledString::as_str()`].
///
/// # Examples
///
/// See [crate-level docs](crate) for the examples of usage.
#[derive(Clone, Copy, Default, PartialEq)]
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
    pub const fn is_plain(&self) -> bool {
        if self.spans.len() > 1 {
            return false;
        }
        match self.spans.get(0) {
            None => true,
            Some(span) => span.style.is_plain(),
        }
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

    /// Returns a slice of this string. This works similarly to [`str::get()`] and returns `None`
    /// under the same conditions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// let styled = styled!("[[green]]Hello, [[it]]world[[/]]!");
    /// let slice = styled.get(3..=8).unwrap();
    /// assert_eq!(slice, styled!("[[green]]lo, [[it]]wo"));
    ///
    /// let slice = styled.get(10..).unwrap();
    /// assert_eq!(slice, styled!("[[it]]ld[[/]]!"));
    /// ```
    pub fn get(&self, range: impl ops::RangeBounds<usize>) -> Option<Self> {
        let range = (
            range.start_bound().map(|&val| val),
            range.end_bound().map(|&val| val),
        );
        Some(Self {
            text: self.text.get(range)?,
            spans: self.spans.get_by_text_range(range),
        })
    }

    /// Splits this string into two at the specified position.
    ///
    /// # Panics
    ///
    /// Panics in the same situations as [`str::split_at()`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// let styled = styled!("[[green]]Hello, [[it]]world[[/]]!");
    /// let (start, end) = styled.split_at(5);
    /// assert_eq!(start, styled!("[[green]]Hello"));
    /// assert_eq!(end, styled!("[[green]], [[it]]world[[/]]!"));
    /// ```
    pub const fn split_at(self, mid: usize) -> (Self, Self) {
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::{styled, SpanStr};
    /// # use anstyle::AnsiColor;
    /// let styled = styled!("[[green]]Hello, [[* bold]]world");
    /// let mut spans = styled.spans();
    /// assert_eq!(spans.len(), 2);
    ///
    /// assert_eq!(
    ///     spans.next().unwrap(),
    ///     SpanStr::new("Hello, ", AnsiColor::Green.on_default())
    /// );
    /// assert_eq!(
    ///     spans.next().unwrap(),
    ///     SpanStr::new("world", AnsiColor::Green.on_default().bold())
    /// );
    /// ```
    pub fn spans(&self) -> impl ExactSizeIterator<Item = SpanStr<'a>> + DoubleEndedIterator + 'a {
        self.spans
            .iter()
            .map(|span| Self::map_span(self.text, span))
    }

    const fn map_span(text: &'a str, span: StyledSpan) -> SpanStr<'a> {
        SpanStr {
            text: utils::const_slice_unchecked(text, span.start..span.end()),
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
    pub const fn span(&self, span_idx: usize) -> Option<SpanStr<'a>> {
        let Some(span) = self.spans.get(span_idx) else {
            return None;
        };
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
    pub const fn span_at(&self, text_pos: usize) -> Option<SpanStr<'a>> {
        if text_pos >= self.text.len() {
            return None;
        }
        let Some(span) = self.spans.get_by_text_pos(text_pos) else {
            return None;
        };
        Some(Self::map_span(self.text, span))
    }

    /// Splits this text by lines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// let styled = styled!("[[bold green!]]Hello,\n  :[[* -color]]world\n");
    /// let lines: Vec<_> = styled.lines().collect();
    /// assert_eq!(lines, [
    ///     styled!("[[bold green!]]Hello,"),
    ///     styled!("[[bold green!]]  :[[bold]]world"),
    /// ]);
    /// ```
    pub fn lines(self) -> Lines<'a> {
        Lines::new(self)
    }

    /// Returns a string with embedded ANSI escapes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::{styled, StyledString};
    /// let styled = styled!("[[bold blue!]]INFO[[/]] [[dim it]](2 min ago)[[/]] Important");
    /// let ansi_str = styled.ansi().to_string();
    /// assert!(ansi_str.contains('\u{1b}'));
    ///
    /// // The ANSI string can be parsed back via `from_ansi()`.
    /// let restored = StyledString::from_ansi(&ansi_str)?;
    /// assert_eq!(restored, styled);
    /// # anyhow::Ok(())
    /// ```
    pub fn ansi(&self) -> impl fmt::Display + '_ {
        Ansi(*self)
    }

    /// Pops a single char from the end of the string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// # use anstyle::Style;
    /// let mut styled = styled!("[[bold green!]]Hello[[it]]!❤");
    /// let (ch, style) = styled.pop().unwrap();
    /// assert_eq!(ch, '❤');
    /// assert_eq!(style, Style::new().italic());
    /// assert_eq!(styled, styled!("[[bold green!]]Hello[[it]]!"));
    ///
    /// styled.pop().unwrap();
    /// assert_eq!(styled, styled!("[[bold green!]]Hello"));
    /// ```
    pub fn pop(&mut self) -> Option<(char, Style)> {
        let ch = self.text.chars().next_back()?;
        self.text = &self.text[..self.text.len() - ch.len_utf8()];
        let style = self.spans.pop_char(ch.len_utf8());
        Some((ch, style))
    }

    /// Converts this string to the owned variant.
    ///
    /// Note that this shadows [`ToOwned::to_owned()`], but this shouldn't be an issue since `StyledStr`
    /// implements [`Copy`].
    pub fn to_owned(self) -> StyledString {
        self.into()
    }

    fn format(&self, formatter: &mut fmt::Formatter<'_>, escape_chars: bool) -> fmt::Result {
        for (i, span) in self.spans.iter().enumerate() {
            let text = &self.text[span.start..span.end()];
            if i == 0 && span.style.is_plain() {
                // Special case: do not output an extra `[[/]]` at the string start.
                write!(formatter, "{}", EscapedText::new(text, escape_chars))?;
            } else {
                write!(
                    formatter,
                    "[[{style}]]{text}",
                    style = RichStyle(&span.style),
                    text = EscapedText::new(text, escape_chars)
                )?;
            }
        }
        Ok(())
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

impl fmt::Debug for StyledStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("\"")?;
        self.format(formatter, true)?;
        formatter.write_str("\"")
    }
}

/// Outputs a string with rich syntax.
impl fmt::Display for StyledStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format(formatter, false)
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
