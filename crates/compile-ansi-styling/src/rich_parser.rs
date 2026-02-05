//! Rich style parsing (incl. in compile time).

use core::ops;

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};

use crate::{
    ParseError, ParseErrorKind, StackStyled, Styled, StyledSpan,
    utils::{Stack, StackStr, StrCursor},
};

impl Styled {
    #[doc(hidden)] // used in the `styled!` macro; logically private
    pub const fn capacities(raw: &str) -> (usize, usize) {
        let mut cursor = StrCursor::new(raw);
        let mut text_len = 0;
        let mut span_count = 0;
        let mut style_end_pos = 0;
        while !cursor.is_eof() {
            if cursor.gobble("[[") {
                let end_pos = cursor.pos() - 2;
                if end_pos - style_end_pos > 0 {
                    span_count += 1;
                }
                while !cursor.gobble("]]") {
                    cursor.advance_byte();
                }
                style_end_pos = cursor.pos();
            } else {
                text_len += 1;
                cursor.advance_byte();
            }
        }

        if raw.len() - style_end_pos > 0 {
            span_count += 1;
        }
        (text_len, span_count)
    }
}

/// Parser for `rich` like styling, e.g. `[[bold red on white]]text[[]]`.
#[derive(Debug)]
struct RichParser<'a> {
    cursor: StrCursor<'a>,
    current_style: Style,
    style_end_pos: usize,
}

impl<'a> RichParser<'a> {
    const fn new(raw: &'a str) -> Self {
        Self {
            cursor: StrCursor::new(raw),
            current_style: Style::new(),
            style_end_pos: 0,
        }
    }

    const fn step(&mut self) -> Result<ops::ControlFlow<(), (StyledSpan, usize)>, ParseError> {
        if self.cursor.is_eof() {
            // Push the final span if necessary.
            return Ok(self.final_step());
        }

        while !self.cursor.is_eof() {
            if self.cursor.gobble("[[") {
                // Push the previous styled span
                let end_pos = self.cursor.pos() - 2;
                let span_and_text_start = if end_pos - self.style_end_pos > 0 {
                    let span = StyledSpan {
                        style: self.current_style,
                        len: end_pos - self.style_end_pos,
                    };
                    Some((span, self.style_end_pos))
                } else {
                    None
                };

                self.current_style = const_try!(self.cursor.parse_style());
                self.style_end_pos = self.cursor.pos();

                if let Some(span_and_text_start) = span_and_text_start {
                    return Ok(ops::ControlFlow::Continue(span_and_text_start));
                }
            } else {
                self.cursor.advance_byte();
            }
        }

        Ok(self.final_step())
    }

    const fn final_step(&mut self) -> ops::ControlFlow<(), (StyledSpan, usize)> {
        if self.cursor.pos() - self.style_end_pos > 0 {
            let span = StyledSpan {
                style: self.current_style,
                len: self.cursor.pos() - self.style_end_pos,
            };
            let text_start = self.style_end_pos;
            self.style_end_pos = self.cursor.pos();
            ops::ControlFlow::Continue((span, text_start))
        } else {
            ops::ControlFlow::Break(())
        }
    }
}

impl StrCursor<'_> {
    const fn error(&self, kind: ParseErrorKind) -> ParseError {
        kind.with_pos(self.pos()..self.pos() + 1)
    }

    /// The cursor is positioned just after the opening `[[`.
    const fn parse_style(&mut self) -> Result<Style, ParseError> {
        let mut style = Style::new();
        let mut is_initial = true;
        while !self.is_eof() && !self.gobble("]]") {
            self.skip_whitespace(is_initial);
            if self.is_eof() {
                return Err(self.error(ParseErrorKind::UnfinishedStyle));
            }

            let token_range = self.take_token();
            let token = self.range(&token_range);

            match token {
                b"bold" | b"b" => {
                    style = style.bold();
                }
                b"italic" | b"i" => {
                    style = style.italic();
                }
                b"underline" | b"u" | b"ul" => {
                    style = style.underline();
                }
                b"strikethrough" | b"strike" | b"s" => {
                    style = style.strikethrough();
                }
                b"dim" | b"dimmed" => {
                    style = style.dimmed();
                }
                b"invert" | b"inverted" | b"inv" => {
                    style = style.invert();
                }
                b"blink" => {
                    style = style.blink();
                }
                b"hide" | b"hidden" | b"conceal" | b"concealed" => {
                    style = style.hidden();
                }

                b"on" => {
                    if style.get_bg_color().is_some() {
                        return Err(ParseErrorKind::RedefinedBackground.with_pos(token_range));
                    }

                    self.skip_whitespace(false);
                    if self.is_eof() {
                        return Err(self.error(ParseErrorKind::UnfinishedStyle));
                    }

                    let token_range = self.take_token();
                    let token = self.range(&token_range);
                    let color = match Self::parse_color(token) {
                        Ok(Some(color)) => color,
                        Ok(None) => {
                            return Err(ParseErrorKind::UnfinishedBackground.with_pos(token_range));
                        }
                        Err(err) => return Err(err.with_pos(token_range)),
                    };
                    style = style.bg_color(Some(color));
                }

                _ => {
                    let color = match Self::parse_color(token) {
                        Ok(Some(color)) => color,
                        Ok(None) => {
                            return Err(ParseErrorKind::UnsupportedStyle.with_pos(token_range));
                        }
                        Err(err) => return Err(err.with_pos(token_range)),
                    };
                    style = style.fg_color(Some(color));
                }
            }

            is_initial = false;
        }
        Ok(style)
    }

    const fn parse_hex_digit(ch: u8) -> Result<u8, ParseErrorKind> {
        match ch {
            b'0'..=b'9' => Ok(ch - b'0'),
            b'a'..=b'f' => Ok(ch - b'a' + 10),
            b'A'..=b'F' => Ok(ch - b'A' + 10),
            _ => Err(ParseErrorKind::InvalidHexColor),
        }
    }

    const fn parse_index(s: &[u8]) -> Result<u8, ParseErrorKind> {
        if s.len() > 3 {
            return Err(ParseErrorKind::InvalidIndexColor);
        }
        let mut i = 0;
        let mut index = 0_u8;
        while i < s.len() {
            let digit = if s[i].is_ascii_digit() {
                s[i] - b'0'
            } else {
                return Err(ParseErrorKind::InvalidIndexColor);
            };
            index = match index.checked_mul(10) {
                Some(val) => val,
                None => return Err(ParseErrorKind::InvalidIndexColor),
            };
            index = match index.checked_add(digit) {
                Some(val) => val,
                None => return Err(ParseErrorKind::InvalidIndexColor),
            };
            i += 1;
        }
        Ok(index)
    }

    const fn parse_color(token: &[u8]) -> Result<Option<Color>, ParseErrorKind> {
        Ok(match token {
            b"black" => Some(Color::Ansi(AnsiColor::Black)),
            b"black*" => Some(Color::Ansi(AnsiColor::BrightBlack)),
            b"red" => Some(Color::Ansi(AnsiColor::Red)),
            b"red*" => Some(Color::Ansi(AnsiColor::BrightRed)),
            b"green" => Some(Color::Ansi(AnsiColor::Green)),
            b"green*" => Some(Color::Ansi(AnsiColor::BrightGreen)),
            b"yellow" => Some(Color::Ansi(AnsiColor::Yellow)),
            b"yellow*" => Some(Color::Ansi(AnsiColor::BrightYellow)),
            b"blue" => Some(Color::Ansi(AnsiColor::Blue)),
            b"blue*" => Some(Color::Ansi(AnsiColor::BrightBlue)),
            b"magenta" => Some(Color::Ansi(AnsiColor::Magenta)),
            b"magenta*" => Some(Color::Ansi(AnsiColor::BrightMagenta)),
            b"cyan" => Some(Color::Ansi(AnsiColor::Cyan)),
            b"cyan*" => Some(Color::Ansi(AnsiColor::BrightCyan)),
            b"white" => Some(Color::Ansi(AnsiColor::White)),
            b"white*" => Some(Color::Ansi(AnsiColor::BrightWhite)),

            hex if !hex.is_empty() && hex[0] == b'#' => {
                if hex.len() == 4 {
                    let r = const_try!(Self::parse_hex_digit(hex[1]));
                    let g = const_try!(Self::parse_hex_digit(hex[2]));
                    let b = const_try!(Self::parse_hex_digit(hex[3]));
                    Some(Color::Rgb(RgbColor(r * 17, g * 17, b * 17)))
                } else if hex.len() == 7 {
                    let r = const_try!(Self::parse_hex_digit(hex[1])) * 16
                        + const_try!(Self::parse_hex_digit(hex[2]));
                    let g = const_try!(Self::parse_hex_digit(hex[3])) * 16
                        + const_try!(Self::parse_hex_digit(hex[4]));
                    let b = const_try!(Self::parse_hex_digit(hex[5])) * 16
                        + const_try!(Self::parse_hex_digit(hex[6]));
                    Some(Color::Rgb(RgbColor(r, g, b)))
                } else {
                    return Err(ParseErrorKind::InvalidHexColor);
                }
            }

            num if !num.is_empty() && num[0].is_ascii_digit() => {
                let index = const_try!(Self::parse_index(num));
                Some(Color::Ansi256(Ansi256Color(index)))
            }

            _ => None,
        })
    }

    const fn skip_whitespace(&mut self, is_initial: bool) {
        while !self.is_eof() {
            let ch = self.current_byte();
            if ch.is_ascii_whitespace() || (!is_initial && matches!(ch, b',' | b';')) {
                self.advance_byte();
            } else {
                break;
            }
        }
    }

    const fn take_token(&mut self) -> ops::Range<usize> {
        const fn is_delimiter(ch: u8) -> bool {
            ch.is_ascii_whitespace() || matches!(ch, b',' | b';' | b']')
        }

        let start_pos = self.pos();
        while !self.is_eof() && !is_delimiter(self.current_byte()) {
            self.advance_byte();
        }
        start_pos..self.pos()
    }
}

impl<const TEXT_CAP: usize, const SPAN_CAP: usize> StackStyled<TEXT_CAP, SPAN_CAP> {
    pub(crate) const fn parse(raw: &str) -> Result<Self, ParseError> {
        let mut parser = RichParser::new(raw);
        let mut text = StackStr::new();
        let mut spans = Stack::new(StyledSpan {
            style: Style::new(),
            len: 0,
        });

        while let ops::ControlFlow::Continue((span, text_start)) = const_try!(parser.step()) {
            if spans.push(span).is_err() {
                return Err(parser.cursor.error(ParseErrorKind::SpanOverflow));
            }

            let text_chunk = parser.cursor.range(&(text_start..text_start + span.len));
            let mut i = 0;
            while i < text_chunk.len() {
                if text.push(text_chunk[i]).is_err() {
                    return Err(parser.cursor.error(ParseErrorKind::TextOverflow));
                }
                i += 1;
            }
        }
        Ok(Self { text, spans })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenization_works() {
        let mut cursor = StrCursor::new("bold, ul");
        let token = cursor.take_token();
        assert_eq!(token, 0..4);
        assert_eq!(cursor.pos(), 4);

        let mut cursor = StrCursor::new("bold");
        let token = cursor.take_token();
        assert_eq!(token, 0..4);
        assert_eq!(cursor.pos(), 4);

        let mut cursor = StrCursor::new("bold]]");
        let token = cursor.take_token();
        assert_eq!(token, 0..4);
        assert_eq!(cursor.pos(), 4);
    }

    #[test]
    fn parsing_style() {
        let mut cursor = StrCursor::new("bold, ul magenta on yellow*");
        let style = cursor.parse_style().unwrap();
        let expected_style = Style::new()
            .bold()
            .underline()
            .fg_color(Some(AnsiColor::Magenta.into()))
            .bg_color(Some(AnsiColor::BrightYellow.into()));
        assert_eq!(style, expected_style);
        assert!(cursor.is_eof(), "{cursor:?}");
    }

    #[test]
    fn parsing_style_with_complex_colors() {
        let mut cursor = StrCursor::new("dim i invert; blink; 42 on #c0ffee");
        let style = cursor.parse_style().unwrap();
        let expected_style = Style::new()
            .dimmed()
            .blink()
            .invert()
            .italic()
            .fg_color(Some(Ansi256Color(42).into()))
            .bg_color(Some(RgbColor(0xc0, 0xff, 0xee).into()));
        assert_eq!(style, expected_style);
        assert!(cursor.is_eof(), "{cursor:?}");
    }
}
