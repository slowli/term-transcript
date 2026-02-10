use super::StyledSpan;

/// Internal trait for types that can be cheaply converted to a [`SpansSlice`].
pub trait AsSpansSlice: Clone + Default {
    /// Performs the conversion.
    fn as_slice(&self) -> SpansSlice<'_>;
}

/// Owned container for styled spans.
#[derive(Debug, Clone, Default)]
pub struct SpansVec(pub(crate) Vec<StyledSpan>);

impl SpansVec {
    pub(crate) const EMPTY: Self = Self(Vec::new());
}

impl AsSpansSlice for SpansVec {
    fn as_slice(&self) -> SpansSlice<'_> {
        SpansSlice::new(&self.0)
    }
}

/// Borrowed slice of styled spans.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpansSlice<'a> {
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
        if i == 0 {
            span.shrink_len(self.text_start - span.start);
        }
        if i + 1 == self.inner.len() {
            span.shrink_len(span.end() - self.text_end);
        }
        span.start = span.start.saturating_sub(self.text_start);
    }

    pub(crate) fn get(&self, index: usize) -> Option<StyledSpan> {
        let mut span = *self.inner.get(index)?;
        self.transform_span(&mut span, index);
        Some(span)
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

    #[cfg(test)]
    pub(crate) fn as_full_slice(self) -> &'a [StyledSpan] {
        assert_eq!(
            self.text_start,
            self.inner.first().map_or(0, StyledSpan::end)
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
        let span_idx = self
            .inner
            .binary_search_by_key(&mid_in_original_text, StyledSpan::end);
        let (start_spans, end_spans) = match span_idx {
            // `mid` is at the span boundary
            Ok(idx) => self.inner.split_at(idx),
            // `mid` is not at the boundary, so span #idx should be included in both slices
            Err(idx) => (&self.inner[..=idx], &self.inner[idx..]),
        };

        let start = Self {
            inner: start_spans,
            text_start: self.text_start,
            text_end: mid,
        };
        let end = Self {
            inner: end_spans,
            text_start: mid,
            text_end: self.text_end,
        };
        (start, end)
    }
}

impl PartialEq for SpansSlice<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.len() == other.inner.len() && self.iter().eq(other.iter())
    }
}

impl AsSpansSlice for SpansSlice<'_> {
    fn as_slice(&self) -> SpansSlice<'_> {
        *self
    }
}
