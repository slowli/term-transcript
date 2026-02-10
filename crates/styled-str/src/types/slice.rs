use anstyle::Style;

use super::spans::StyledSpan;

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

    fn transform_span(&self, span: &mut StyledSpan, i: usize) {
        if i + 1 == self.inner.len() {
            // Must be done first because `span.end()` is affected by `shrink_len()`.
            span.shrink_len(span.end() - self.text_end);
        }
        if i == 0 {
            span.shrink_len(self.text_start - span.start);
        }
        span.start = span.start.saturating_sub(self.text_start);
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<StyledSpan> {
        let mut span = *self.inner.get(index)?;
        self.transform_span(&mut span, index);
        Some(span)
    }

    pub(crate) fn get_by_text_pos(&self, text_pos: usize) -> Option<StyledSpan> {
        let pos_in_original_text = text_pos + self.text_start;
        let idx = self
            .inner
            .binary_search_by_key(&pos_in_original_text, |span| span.start);
        let idx = idx.unwrap_or_else(|idx| idx - 1);
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

    pub(crate) fn split_at(&self, mid: usize) -> (Self, Self) {
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
            let span_idx = self
                .inner
                .binary_search_by_key(&mid_in_original_text, StyledSpan::end);
            match span_idx {
                // `mid` is at the span boundary
                Ok(idx) => self.inner.split_at(idx + 1),
                // `mid` is not at the boundary, so span #idx should be included in both slices
                Err(idx) => (&self.inner[..=idx], &self.inner[idx..]),
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
