//! `SpansSlice`.

use crate::StyledSpan;

#[derive(Debug, Clone, Copy)]
pub struct SpansSlice<'a> {
    inner: &'a [StyledSpan],
    first_span_len: Option<usize>,
    last_span_len: Option<usize>,
}

impl<'a> SpansSlice<'a> {
    const EMPTY: Self = Self::new(&[]);

    pub const fn new(spans: &'a [StyledSpan]) -> Self {
        Self {
            inner: spans,
            first_span_len: None,
            last_span_len: None,
        }
    }

    fn span_len(&self, span: &StyledSpan, i: usize) -> usize {
        let mut len_override = None;

        // `last_span_len` by design has higher priority compared to `first_span_len` if they both
        // concern the same span.
        if i + 1 == self.inner.len() {
            len_override = self.last_span_len;
        }
        if i == 0 {
            len_override = len_override.or(self.first_span_len);
        }
        len_override.unwrap_or(span.len)
    }

    #[must_use]
    pub fn split_off(&mut self, pos: usize) -> Self {
        let mut total_len = 0;
        for (i, span) in self.inner.iter().enumerate() {
            let effective_len = self.span_len(span, i);
            total_len += effective_len;
            if total_len > pos {
                let tail = Self {
                    inner: &self.inner[i..],
                    first_span_len: Some(total_len - pos),
                    last_span_len: None,
                };

                let last_span_consumed_len = pos - (total_len - effective_len);
                if last_span_consumed_len > 0 {
                    self.inner = &self.inner[..=i];
                    self.last_span_len = Some(last_span_consumed_len);
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
                len: 5,
                style: Style::new(),
            },
            StyledSpan {
                len: 3,
                style: Style::new().bold(),
            },
        ];
        let mut spans = SpansSlice::new(spans);

        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [
                StyledSpan {
                    len: 5,
                    style: Style::new()
                },
                StyledSpan {
                    len: 3,
                    style: Style::new().bold()
                },
            ]
        );

        let tail = spans.split_off(6);
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan {
                len: 2,
                style: Style::new().bold()
            }]
        );
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [
                StyledSpan {
                    len: 5,
                    style: Style::new()
                },
                StyledSpan {
                    len: 1,
                    style: Style::new().bold()
                },
            ]
        );

        let tail = spans.split_off(5);
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan {
                len: 1,
                style: Style::new().bold()
            }]
        );
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [StyledSpan {
                len: 5,
                style: Style::new()
            },]
        );

        let tail = spans.split_off(3);
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan {
                len: 2,
                style: Style::new()
            }]
        );
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            [StyledSpan {
                len: 3,
                style: Style::new()
            }]
        );
    }

    #[test]
    fn splitting_span_from_both_sides() {
        let spans = &[StyledSpan {
            len: 5,
            style: Style::new(),
        }];
        let mut spans = SpansSlice::new(spans);
        let mut tail = spans.split_off(2);
        let _ = tail.split_off(1);

        assert_eq!(tail.first_span_len, Some(3));
        assert_eq!(tail.last_span_len, Some(1));
        assert_eq!(
            tail.iter().collect::<Vec<_>>(),
            [StyledSpan {
                len: 1,
                style: Style::new()
            }]
        );
    }
}
