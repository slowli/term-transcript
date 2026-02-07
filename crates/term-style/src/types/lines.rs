//! `Lines` iterator.

use core::mem;

use crate::{Styled, StyledSpan, StyledStr};

#[derive(Debug)]
pub struct Lines<'a> {
    text: &'a str,
    spans: &'a [StyledSpan],
    first_span_len: usize,
}

impl<'a> Lines<'a> {
    pub(super) fn new(str: StyledStr<'a>) -> Self {
        Self {
            text: str.text,
            spans: str.spans,
            first_span_len: str.spans.first().map_or(0, |span| span.len),
        }
    }

    fn take_spans(&mut self, text_len: usize) -> Vec<StyledSpan> {
        assert!(text_len > 0);

        let mut total_len = 0;
        for (i, span) in self.spans.iter().enumerate() {
            let effective_len = if i == 0 {
                debug_assert!(self.first_span_len > 0);
                debug_assert!(self.first_span_len <= span.len);
                self.first_span_len
            } else {
                span.len
            };

            total_len += effective_len;
            if total_len > text_len {
                let (head, tail) = self.spans.split_at(i);
                let mut output = head.to_vec();
                if let Some(first) = output.first_mut() {
                    first.len = self.first_span_len;
                }

                let last_span_consumed_len = text_len - (total_len - effective_len);
                if last_span_consumed_len > 0 {
                    output.push(StyledSpan {
                        style: span.style,
                        len: last_span_consumed_len,
                    });
                }

                self.spans = tail;
                self.first_span_len = total_len - text_len;
                return output;
            }
        }

        // If we're here, `text_len` covers all text
        let mut spans = mem::take(&mut self.spans).to_vec();
        if let Some(first) = spans.first_mut() {
            first.len = self.first_span_len;
        }
        spans
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = Styled<&'a str, Vec<StyledSpan>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.text.is_empty() {
            return None;
        }

        // Find the next '\n' occurrence in `str`
        let next_pos = self.text.find('\n').map_or(self.text.len(), |pos| pos + 1);
        let (line, tail) = self.text.split_at(next_pos);
        self.text = tail;
        let spans = self.take_spans(line.len());

        let mut line = Styled { text: line, spans };
        // Pop the ending `\n` and `\r`, same as `lines()` iterator for `str` does.
        if line.text.ends_with('\n') {
            line.pop();
        }
        if line.text.ends_with('\r') {
            line.pop();
        }
        Some(line)
    }
}

#[cfg(test)]
mod tests {
    use crate::styled;

    #[test]
    fn lines_basics() {
        let str = styled!("[[red on white]]Test");
        let lines: Vec<_> = str.lines().collect();
        assert_eq!(lines, [str]);

        let str_with_nl = styled!("[[red on white]]Test\n");
        let lines: Vec<_> = str_with_nl.lines().collect();
        assert_eq!(lines, [str]);

        let str_with_nl = styled!("[[red on white]]Test\r\n");
        let lines: Vec<_> = str_with_nl.lines().collect();
        assert_eq!(lines, [str]);
    }

    #[test]
    fn lines_with_multiline_text() {
        let str = styled!("[[red on white]]Test\nHello, [[bold green]]world[[* -bold]]!");
        let expected_lines = [
            styled!("[[red on white]]Test"),
            styled!("[[red on white]]Hello, [[bold green]]world[[green]]!"),
        ];

        let lines: Vec<_> = str.lines().collect();
        assert_eq!(lines, expected_lines);
    }

    #[test]
    fn styles_bordering_on_newlines() {
        let str = styled!("[[red on white]]Test\n[[]]Hello,[[bold green]]\nworld[[* -bold]]!\n");
        let expected_lines = [
            styled!("[[red on white]]Test"),
            styled!("Hello,"),
            styled!("[[bold green]]world[[green]]!"),
        ];

        let lines: Vec<_> = str.lines().collect();
        assert_eq!(lines, expected_lines);
    }
}
