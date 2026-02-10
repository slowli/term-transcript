//! `Lines` iterator.

use core::mem;

use crate::{Styled, StyledSpan, StyledStr, alloc::Vec};

/// Iterator over lines in a [`StyledStr`]. Returned by [`StyledStr::lines()`].
#[derive(Debug)]
pub struct Lines<'a> {
    text: &'a str,
    spans: SpansSlice<'a>,
}

impl<'a> Lines<'a> {
    pub(super) fn new(str: StyledStr<'a>) -> Self {
        Self {
            text: str.text,
            spans: SpansSlice::new(str.spans),
        }
    }

    fn take_spans(&mut self, text_len: usize) -> Vec<StyledSpan> {
        assert!(text_len > 0);
        let tail = self.spans.split_off(text_len);
        mem::replace(&mut self.spans, tail).to_vec()
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
        let str = styled!("[[red on white]]Test\n[[/]]Hello,[[bold green]]\nworld[[* -bold]]!\n");
        let expected_lines = [
            styled!("[[red on white]]Test"),
            styled!("Hello,"),
            styled!("[[bold green]]world[[green]]!"),
        ];

        let lines: Vec<_> = str.lines().collect();
        assert_eq!(lines, expected_lines);
    }
}
