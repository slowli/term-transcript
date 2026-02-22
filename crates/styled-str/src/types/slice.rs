use core::ops;

use anstyle::Style;

use super::spans::StyledSpan;

#[derive(Debug, Clone, Copy)]
enum Bound {
    Start,
    End,
}

const fn binary_search_spans(
    spans: &[StyledSpan],
    pos: usize,
    bound: Bound,
) -> Result<usize, usize> {
    let mut lo = 0;
    let mut hi = spans.len();
    while lo < hi {
        let mid = usize::midpoint(lo, hi);
        let mid_value = match bound {
            Bound::Start => spans[mid].start,
            Bound::End => spans[mid].end(),
        };
        if mid_value == pos {
            return Ok(mid);
        } else if mid_value < pos {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Err(lo)
}

/// Borrowed slice of styled spans used by [`StyledStr`](crate::StyledStr).
///
/// # Implementation notes
///
/// A separate type is used as opposed to a slice `&[_]` both to achieve better encapsulation,
/// and because we want to not copy span locations when slicing styled strings (including in a middle
/// of a span). The latter requires maintaining info *in addition* to span locations.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SpansSlice<'a> {
    inner: &'a [StyledSpan],
    /// Byte positions in the sliced plaintext. It would be more idiomatic to use `ops::Range<usize>`,
    /// but it doesn't implement `Copy`.
    text_start: usize,
    text_end: usize,
}

impl<'a> SpansSlice<'a> {
    const EMPTY: Self = Self::new(&[]);

    pub(crate) const fn new(inner: &'a [StyledSpan]) -> Self {
        Self {
            inner,
            text_start: 0,
            text_end: if let Some(last_span) = inner.last() {
                last_span.end()
            } else {
                0
            },
        }
    }

    const fn transform_span(&self, span: &mut StyledSpan, i: usize) {
        if i + 1 == self.inner.len() {
            // Must be done first because `span.end()` is affected by `shrink_len()`.
            span.shrink_len(span.end() - self.text_end);
        }
        if i == 0 {
            span.shrink_len(self.text_start - span.start);
        }
        span.start = span.start.saturating_sub(self.text_start);
    }

    pub(crate) const fn len(&self) -> usize {
        self.inner.len()
    }

    pub(crate) const fn get(&self, index: usize) -> Option<StyledSpan> {
        let mut span = if index < self.inner.len() {
            self.inner[index]
        } else {
            return None;
        };
        self.transform_span(&mut span, index);
        Some(span)
    }

    pub(crate) const fn get_by_text_pos(&self, text_pos: usize) -> Option<StyledSpan> {
        let pos_in_original_text = text_pos + self.text_start;
        let idx = binary_search_spans(self.inner, pos_in_original_text, Bound::Start);
        let idx = match idx {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        self.get(idx)
    }

    pub(crate) fn iter(
        self,
    ) -> impl ExactSizeIterator<Item = StyledSpan> + DoubleEndedIterator + 'a {
        self.inner
            .iter()
            .copied()
            .enumerate()
            .map(move |(i, mut span)| {
                self.transform_span(&mut span, i);
                span
            })
    }

    /// Returns the underlying slice, but only if the text boundaries correspond to the slices.
    #[cfg(test)]
    pub(crate) fn as_full_slice(self) -> &'a [StyledSpan] {
        assert_eq!(
            self.text_start,
            self.inner.first().map_or(0, |span| span.start)
        );
        assert_eq!(self.text_end, self.inner.last().map_or(0, StyledSpan::end));
        self.inner
    }

    /// The range is assumed to be valid w.r.t. the text.
    pub(crate) fn get_by_text_range(
        self,
        (start, end): (ops::Bound<usize>, ops::Bound<usize>),
    ) -> Self {
        let start = match start {
            ops::Bound::Unbounded => 0,
            ops::Bound::Excluded(pos) => pos + 1,
            ops::Bound::Included(pos) => pos,
        };
        let start = start + self.text_start;

        let end = match end {
            ops::Bound::Unbounded => self.text_end,
            ops::Bound::Excluded(pos) => self.text_start + pos,
            ops::Bound::Included(pos) => self.text_start + pos + 1,
        };
        if start >= end {
            return Self::EMPTY;
        }

        let start_idx = binary_search_spans(self.inner, start, Bound::Start);
        let start_idx = start_idx.unwrap_or_else(|idx| idx - 1);
        let end_idx = binary_search_spans(&self.inner[start_idx..], end, Bound::End);
        // We need to add 1 because the end index is exclusive, and `start_idx` because the index
        // is returned for a subslice.
        let end_idx = end_idx.unwrap_or_else(|idx| idx) + 1 + start_idx;
        Self {
            inner: &self.inner[start_idx..end_idx],
            text_start: start,
            text_end: end,
        }
    }

    pub(crate) fn start_with(&self, needle: &SpansSlice<'_>) -> bool {
        let needle_len = needle.len();
        if needle_len > self.len() {
            return false;
        }

        needle
            .iter()
            .zip(self.iter())
            .enumerate()
            .all(|(i, (needle_span, this_span))| {
                this_span.style == needle_span.style && {
                    let cmp = this_span.len.cmp(&needle_span.len);
                    cmp.is_eq() || (i + 1 == needle_len && cmp.is_gt())
                }
            })
    }

    pub(crate) fn end_with(&self, needle: &SpansSlice<'_>) -> bool {
        let needle_len = needle.len();
        if needle_len > self.len() {
            return false;
        }

        needle.iter().rev().zip(self.iter().rev()).enumerate().all(
            |(i, (needle_span, this_span))| {
                this_span.style == needle_span.style && {
                    let cmp = this_span.len.cmp(&needle_span.len);
                    cmp.is_eq() || (i + 1 == needle_len && cmp.is_gt())
                }
            },
        )
    }

    pub(crate) const fn split_at(&self, mid: usize) -> (Self, Self) {
        assert!(
            mid <= self.text_end - self.text_start,
            "`mid` is out of bounds"
        );

        let mid_in_original_text = self.text_start + mid;
        let (start_spans, end_spans) = if mid_in_original_text == 0 {
            // Special case; the general logic would always return at least the first item from `self.inner`
            // in `start_spans`.
            (&[] as &[_], self.inner)
        } else {
            let span_idx = binary_search_spans(self.inner, mid_in_original_text, Bound::End);
            match span_idx {
                // `mid` is at the span boundary
                Ok(idx) => self.inner.split_at(idx + 1),
                // `mid` is not at the boundary, so span #idx should be included in both slices
                Err(idx) => {
                    let (start_spans, _) = self.inner.split_at(idx + 1);
                    let (_, end_spans) = self.inner.split_at(idx);
                    (start_spans, end_spans)
                }
            }
        };

        let start = Self {
            inner: start_spans,
            text_start: self.text_start,
            text_end: mid_in_original_text,
        };
        let end = Self {
            inner: end_spans,
            text_start: mid_in_original_text,
            text_end: self.text_end,
        };
        (start, end)
    }

    pub(crate) fn pop_char(&mut self, char_len: usize) -> Style {
        self.text_end -= char_len;
        assert!(
            self.text_end >= self.text_start,
            "called `pop_char()` with empty spans"
        );
        // `unwrap()` is safe due to the check above
        let last_span = self.inner.last().unwrap();
        if last_span.start >= self.text_end || self.text_start == self.text_end {
            self.inner = &self.inner[..self.inner.len() - 1];
        }
        last_span.style
    }
}

impl PartialEq for SpansSlice<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.len() == other.inner.len() && self.iter().eq(other.iter())
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroUsize;

    use super::*;

    fn span_at(start: usize, len: usize) -> StyledSpan {
        StyledSpan {
            style: Style::new(),
            len: NonZeroUsize::new(len).unwrap(),
            start,
        }
    }

    #[test]
    fn binary_search_works() {
        let spans = [span_at(0, 3), span_at(3, 2), span_at(5, 5), span_at(10, 1)];

        assert_eq!(binary_search_spans(&spans, 0, Bound::Start), Ok(0));
        assert_eq!(binary_search_spans(&spans, 1, Bound::Start), Err(1));
        assert_eq!(binary_search_spans(&spans, 2, Bound::Start), Err(1));
        assert_eq!(binary_search_spans(&spans, 3, Bound::Start), Ok(1));
        assert_eq!(binary_search_spans(&spans, 4, Bound::Start), Err(2));
        assert_eq!(binary_search_spans(&spans, 5, Bound::Start), Ok(2));
        assert_eq!(binary_search_spans(&spans, 6, Bound::Start), Err(3));
        assert_eq!(binary_search_spans(&spans, 8, Bound::Start), Err(3));
        assert_eq!(binary_search_spans(&spans, 10, Bound::Start), Ok(3));
        assert_eq!(binary_search_spans(&spans, 11, Bound::Start), Err(4));

        assert_eq!(binary_search_spans(&spans, 0, Bound::End), Err(0));
        assert_eq!(binary_search_spans(&spans, 1, Bound::End), Err(0));
        assert_eq!(binary_search_spans(&spans, 2, Bound::End), Err(0));
        assert_eq!(binary_search_spans(&spans, 3, Bound::End), Ok(0));
        assert_eq!(binary_search_spans(&spans, 4, Bound::End), Err(1));
        assert_eq!(binary_search_spans(&spans, 5, Bound::End), Ok(1));
        assert_eq!(binary_search_spans(&spans, 6, Bound::End), Err(2));
        assert_eq!(binary_search_spans(&spans, 8, Bound::End), Err(2));
        assert_eq!(binary_search_spans(&spans, 10, Bound::End), Ok(2));
        assert_eq!(binary_search_spans(&spans, 11, Bound::End), Ok(3));
        assert_eq!(binary_search_spans(&spans, 12, Bound::End), Err(4));
    }
}
