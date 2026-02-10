//! `SpansSlice`.

use core::num::NonZeroUsize;

use crate::{StyledSpan, alloc::Vec};


impl<'a> SpansSlice<'a> {
    const EMPTY: Self = Self::new(&[]);

    /// Wraps the provided spans.
    pub const fn new(spans: &'a [StyledSpan]) -> Self {
        Self {
            inner: spans,
            first_span_len: None,
            last_span_len: None,
        }
    }



    /// Splits the spans at the specified byte position in the string.
    #[must_use]
    pub fn split_off(&mut self, pos: usize) -> Self {
        let mut total_len = 0;
        for (i, span) in self.inner.iter().enumerate() {
            let effective_len = self.span_len(span, i).get();
            total_len += effective_len;
            if total_len > pos {
                let tail = Self {
                    inner: &self.inner[i..],
                    first_span_len: NonZeroUsize::new(total_len - pos), // always `Some(_)`
                    last_span_len: None,
                };

                let last_span_consumed_len = pos - (total_len - effective_len);
                if last_span_consumed_len > 0 {
                    self.inner = &self.inner[..=i];
                    self.last_span_len = NonZeroUsize::new(last_span_consumed_len); // always `Some(_)`
                } else {
                    self.inner = &self.inner[..i];
                    self.last_span_len = None;
                }

                return tail;
            }
        }

        // If we're here, `pos` is greater or equal the total span length. Then, `split_off()` is a no-op.
        Self::EMPTY
    }

    /// Iterates over the contained spans.
    pub fn iter(self) -> impl Iterator<Item = StyledSpan> + 'a {
        self.inner
            .iter()
            .copied()
            .enumerate()
            .map(move |(i, mut span)| {
                span.len = self.span_len(&span, i);
                span
            })
    }

    /// Collects these spans to a vector.
    pub fn to_vec(self) -> Vec<StyledSpan> {
        let mut output = self.inner.to_vec();
        let span_count = output.len();
        if let Some(first) = output.first_mut() {
            first.len = self.span_len(first, 0);
        }
        if let Some(last) = output.last_mut() {
            last.len = self.span_len(last, span_count - 1);
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use anstyle::Style;

    use super::*;

    #[test]
    fn spans_slice_basics() {
        let spans = &[
            StyledSpan {
                len: NonZeroUsize::new(5).unwrap(),
                style: Style::new(),
            },
            StyledSpan {
                len: NonZeroUsize::new(3).unwrap(),
                style: Style::new().bold(),
            },
        ];
        let mut spans = SpansSlice::new(spans);

        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [
                StyledSpan::new(Style::new(), 5),
                StyledSpan::new(Style::new().bold(), 3),
            ]
        );

        let tail = spans.split_off(6);
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan::new(Style::new().bold(), 2)]
        );
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [
                StyledSpan::new(Style::new(), 5),
                StyledSpan::new(Style::new().bold(), 1),
            ]
        );

        let tail = spans.split_off(5);
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan::new(Style::new().bold(), 1)]
        );
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [StyledSpan::new(Style::new(), 5)]
        );

        let tail = spans.split_off(3);
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan::new(Style::new(), 2)]
        );
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [StyledSpan::new(Style::new(), 3)]
        );
    }

    #[test]
    fn splitting_span_from_both_sides() {
        let spans = &[StyledSpan::new(Style::new(), 5)];
        let mut spans = SpansSlice::new(spans);
        let mut tail = spans.split_off(2);
        let _ = tail.split_off(1);

        assert_eq!(tail.first_span_len, NonZeroUsize::new(3));
        assert_eq!(tail.last_span_len, NonZeroUsize::new(1));
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan::new(Style::new(), 1)]
        );
    }
}
