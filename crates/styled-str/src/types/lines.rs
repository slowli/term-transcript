//! `Lines` iterator.

use crate::StyledStr;

/// Iterator over lines in a [`StyledStr`]. Returned by [`StyledStr::lines()`].
#[derive(Debug)]
pub struct Lines<'a> {
    remainder: StyledStr<'a>,
}

impl<'a> Lines<'a> {
    pub(super) fn new(str: StyledStr<'a>) -> Self {
        Self { remainder: str }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = StyledStr<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remainder.is_empty() {
            return None;
        }

        // Find the next '\n' occurrence in the text
        let text = self.remainder.text();
        let next_pos = text.find('\n').map_or(text.len(), |pos| pos + 1);
        let (mut line, remainder) = self.remainder.split_at(next_pos);
        self.remainder = remainder;

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
