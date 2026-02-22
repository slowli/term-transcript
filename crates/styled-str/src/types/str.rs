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

    /// Checks whether this string starts with a `needle`, matching both its text and styling.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// let styled = styled!("[[green]]Hello, [[* bold]]world");
    /// assert!(styled.starts_with(styled!("[[green]]Hello")));
    /// // Styling is taken into account
    /// assert!(!styled.starts_with(styled!("Hello")));
    /// ```
    pub fn starts_with(&self, needle: StyledStr<'_>) -> bool {
        self.text.starts_with(needle.text) && self.spans.start_with(&needle.spans)
    }

    /// Checks whether this string ends with a `needle`, matching both its text and styling.
    ///
    /// # Examples
    ///
    /// ```
    /// # use styled_str::styled;
    /// let styled = styled!("[[green]]Hello, [[* bold]]world");
    /// assert!(styled.ends_with(styled!("[[green bold]]ld")));
    /// // Styling is taken into account
    /// assert!(!styled.ends_with(styled!("world")));
    /// ```
    pub fn ends_with(&self, needle: StyledStr<'_>) -> bool {
        self.text.ends_with(needle.text) && self.spans.end_with(&needle.spans)
    }

    /// Checks whether `needle` is contained in this string, matching both by text and styling.
    pub fn contains(&self, needle: StyledStr<'_>) -> bool {
        self.find(needle).is_some()
    }

    /// Finds the first byte position of `needle` in this string from the string start, matching both by text and styling.
    #[allow(clippy::missing_panics_doc)] // Internal check that should never be triggered
    pub fn find(&self, needle: StyledStr<'_>) -> Option<usize> {
        let Some(first_needle_span) = needle.spans.iter().next() else {
            // `needle` is empty
            return Some(0);
        };
        let needle_has_multiple_spans = needle.spans.len() > 1;

        let mut text_matched_on_prev_iteration = false;
        let mut start_pos = 0;
        loop {
            // First, find a candidate by styling by considering the starting span.
            // This is efficient if the styled string doesn't contain many styles.
            let spans_suffix = self
                .spans
                .get_by_text_range((ops::Bound::Included(start_pos), ops::Bound::Unbounded));
            let offset_by_spans = spans_suffix.iter().find_map(|span| {
                span.can_contain(&first_needle_span).then(|| {
                    let mut offset = span.start;
                    if needle_has_multiple_spans {
                        // Need to align the span end.
                        offset += span.len.get() - first_needle_span.len.get();
                    }
                    offset
                })
            });
            let Some(offset_by_spans) = offset_by_spans else {
                // No matching style spans
                return None;
            };
            start_pos += offset_by_spans;
            // We cannot guarantee that `start_pos` is at the char boundary, and the code below demands it.
            start_pos = utils::ceil_char_boundary(self.text.as_bytes(), start_pos);

            let offset_by_text = if offset_by_spans == 0 && text_matched_on_prev_iteration {
                // Can reuse the text match found on the previous iteration
                0
            } else {
                let Some(offset_by_text) = self.text[start_pos..].find(needle.text) else {
                    // No text mentions
                    return None;
                };
                offset_by_text
            };
            start_pos += offset_by_text;

            if offset_by_text == 0 {
                // The text match *may* correspond to the style match; check the spans slice completely.
                let range = (
                    ops::Bound::Included(start_pos),
                    ops::Bound::Excluded(start_pos + needle.text.len()),
                );
                let spans_slice = self.spans.get_by_text_range(range);
                if spans_slice == needle.spans {
                    return Some(start_pos);
                }

                // We guarantee that the first style span matches, so this case can only happen if the needle
                // contains multiple spans. Because the first and second span styles differ, we can advance
                // the search position by first + second span lengths (i.e., at least 2).
                assert!(needle_has_multiple_spans);
                let second_span_len = needle.spans.get(1).unwrap().len;
                let offset = first_needle_span.len.get() + second_span_len.get();
                start_pos += offset;
                start_pos = utils::ceil_char_boundary(self.text.as_bytes(), start_pos);
            }
            // Otherwise, the text match is somewhere after the found style match, so we refine the style match
            // on the next loop iteration.
            text_matched_on_prev_iteration = offset_by_text != 0;
        }
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
