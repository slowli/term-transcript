//! Rich style parsing (incl. in compile time).

use core::{fmt, ops, str::FromStr};

use anstyle::{Ansi256Color, AnsiColor, Color, Effects, RgbColor, Style};

use crate::{
    DynStyled, ParseError, ParseErrorKind, StackStyled, Styled, StyledSpan,
    utils::{Stack, StackStr, StrCursor, is_same_style, normalize_style},
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
                while !cursor.is_eof() && cursor.current_byte() == b'[' {
                    cursor.advance_byte();
                }

                let end_pos = cursor.pos() - 2;
                if end_pos - style_end_pos > 0 {
                    span_count += 1;
                    text_len += end_pos - style_end_pos;
                }
                while !cursor.is_eof() && !cursor.gobble("]]") {
                    cursor.advance_byte();
                }
                style_end_pos = cursor.pos();
            } else {
                cursor.advance_byte();
            }
        }

        if raw.len() - style_end_pos > 0 {
            span_count += 1;
            text_len += raw.len() - style_end_pos;
        }
        (text_len, span_count)
    }
}

#[derive(Debug)]
struct SetStyleFields(u16);

impl SetStyleFields {
    const BOLD: u16 = 1;
    const ITALIC: u16 = 2;
    const UNDERLINE: u16 = 4;
    const STRIKETHROUGH: u16 = 8;
    const BLINK: u16 = 16;
    const INVERT: u16 = 32;
    const HIDDEN: u16 = 64;
    const DIMMED: u16 = 128;

    const FG: u16 = 256;
    const BG: u16 = 512;

    const fn new() -> Self {
        Self(0)
    }

    const fn from_style(style: &Style) -> Self {
        let mut this = Self::new();
        this.set_effects(style.get_effects());
        if style.get_fg_color().is_some() {
            this.set_fg();
        }
        if style.get_bg_color().is_some() {
            this.set_bg();
        }
        this
    }

    const fn set_effects(&mut self, effects: Effects) -> bool {
        let prev = self.0;

        if effects.contains(Effects::BOLD) {
            self.0 |= Self::BOLD;
        }
        if effects.contains(Effects::ITALIC) {
            self.0 |= Self::ITALIC;
        }
        if effects.contains(Effects::UNDERLINE) {
            self.0 |= Self::UNDERLINE;
        }
        if effects.contains(Effects::STRIKETHROUGH) {
            self.0 |= Self::STRIKETHROUGH;
        }
        if effects.contains(Effects::BLINK) {
            self.0 |= Self::BLINK;
        }
        if effects.contains(Effects::INVERT) {
            self.0 |= Self::INVERT;
        }
        if effects.contains(Effects::HIDDEN) {
            self.0 |= Self::HIDDEN;
        }
        if effects.contains(Effects::DIMMED) {
            self.0 |= Self::DIMMED;
        }

        self.0 != prev
    }

    const fn set_fg(&mut self) -> bool {
        let prev = self.0;
        self.0 |= Self::FG;
        self.0 != prev
    }

    const fn set_bg(&mut self) -> bool {
        let prev = self.0;
        self.0 |= Self::BG;
        self.0 != prev
    }
}

/// Parser for `rich` like styling, e.g. `[[bold red on white]]text[[]]`.
// TODO: make the number of opening / closing brackets configurable?
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
                // Gobble any additional `[`s. It's always right to interpret the rightmost sequence of `[`
                // as the start of the style, because the style itself cannot contain `[`s.
                while !self.cursor.is_eof() && self.cursor.current_byte() == b'[' {
                    self.cursor.advance_byte();
                }
                if self.cursor.is_eof() {
                    return Err(self.cursor.error(ParseErrorKind::UnfinishedStyle));
                }

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

                self.current_style = const_try!(self.cursor.parse_style(&self.current_style));
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
    /// Produces the error spanned at the next char.
    const fn error(&self, kind: ParseErrorKind) -> ParseError {
        let range = if self.is_eof() {
            self.pos()..self.pos()
        } else {
            self.expand_to_char_boundaries(self.pos()..self.pos() + 1)
        };
        kind.with_pos(range)
    }

    const fn error_on_empty_token(&self, unfinished: ParseErrorKind) -> ParseError {
        if self.is_eof() {
            unfinished.with_pos(self.pos()..self.pos())
        } else {
            let next_char_pos = self.expand_to_char_boundaries(self.pos()..self.pos() + 1);
            ParseErrorKind::BogusDelimiter.with_pos(next_char_pos)
        }
    }

    /// The cursor is positioned just after the opening `[[`.
    #[allow(clippy::too_many_lines)] // FIXME: split into parts
    const fn parse_style(&mut self, current_style: &Style) -> Result<Style, ParseError> {
        let mut style = Style::new();
        let mut is_initial = true;
        let mut copied_fields = None;
        let mut set_fields = SetStyleFields::new();

        while !self.is_eof() {
            self.gobble_whitespace();
            if !is_initial && self.gobble_punct() {
                self.gobble_whitespace();
            }

            if self.gobble("]]") {
                return Ok(normalize_style(style));
            }

            if self.is_eof() {
                return Err(self.error(ParseErrorKind::UnfinishedStyle));
            }

            let token_range = self.take_token();
            let token = self.range(&token_range);

            match token {
                b"" => {
                    return Err(self.error_on_empty_token(ParseErrorKind::UnfinishedStyle));
                }

                b"*" => {
                    if is_initial {
                        copied_fields = Some(SetStyleFields::from_style(current_style));
                        style = *current_style;
                    } else {
                        return Err(ParseErrorKind::NonInitialCopy.with_pos(token_range));
                    }
                }

                b"on" => {
                    if !set_fields.set_bg() {
                        return Err(ParseErrorKind::DuplicateSpecifier.with_pos(token_range));
                    }

                    self.gobble_whitespace();
                    if self.is_eof() {
                        return Err(self.error(ParseErrorKind::UnfinishedStyle));
                    }

                    let on_token_range = token_range;
                    let token_range = self.take_token();
                    let token = self.range(&token_range);
                    let color = match Self::parse_color(token) {
                        Ok(Some(color)) => color,
                        Ok(None) => {
                            return Err(
                                ParseErrorKind::UnfinishedBackground.with_pos(on_token_range)
                            );
                        }
                        Err(err) => return Err(err.with_pos(token_range)),
                    };
                    style = style.bg_color(Some(color));
                }

                b"-color" | b"-fg" | b"!color" | b"!fg" => {
                    if !set_fields.set_fg() {
                        return Err(ParseErrorKind::DuplicateSpecifier.with_pos(token_range));
                    }
                    let Some(copied_fields) = &mut copied_fields else {
                        // This can be checked earlier, but we'd like to return an `UnsupportedEffect` error
                        // on a bogus specifier.
                        return Err(ParseErrorKind::NegationWithoutCopy.with_pos(token_range));
                    };
                    if copied_fields.set_fg() {
                        // The effect wasn't set in the copied style
                        return Err(ParseErrorKind::RedundantNegation.with_pos(token_range));
                    }
                    style = style.fg_color(None);
                }
                b"-on" | b"-bg" | b"!on" | b"!bg" => {
                    if !set_fields.set_bg() {
                        return Err(ParseErrorKind::DuplicateSpecifier.with_pos(token_range));
                    }
                    let Some(copied_fields) = &mut copied_fields else {
                        // This can be checked earlier, but we'd like to return an `UnsupportedEffect` error
                        // on a bogus specifier.
                        return Err(ParseErrorKind::NegationWithoutCopy.with_pos(token_range));
                    };
                    if copied_fields.set_bg() {
                        // The effect wasn't set in the copied style
                        return Err(ParseErrorKind::RedundantNegation.with_pos(token_range));
                    }
                    style = style.bg_color(None);
                }
                neg if neg[0] == b'-' || neg[0] == b'!' => {
                    let (_, token_without_prefix) = token.split_at(1);
                    let Some(effects) = Self::parse_effects(token_without_prefix) else {
                        return Err(ParseErrorKind::UnsupportedEffect.with_pos(token_range));
                    };
                    let Some(copied_fields) = &mut copied_fields else {
                        // This can be checked earlier, but we'd like to return an `UnsupportedEffect` error
                        // on a bogus specifier.
                        return Err(ParseErrorKind::NegationWithoutCopy.with_pos(token_range));
                    };

                    if !set_fields.set_effects(effects) {
                        return Err(ParseErrorKind::DuplicateSpecifier.with_pos(token_range));
                    }
                    if copied_fields.set_effects(effects) {
                        // The effect wasn't set in the copied style
                        return Err(ParseErrorKind::RedundantNegation.with_pos(token_range));
                    }

                    style = style.effects(style.get_effects().remove(effects));
                }

                _ => {
                    if let Some(effects) = Self::parse_effects(token) {
                        if !set_fields.set_effects(effects) {
                            return Err(ParseErrorKind::DuplicateSpecifier.with_pos(token_range));
                        }
                        style = style.effects(style.get_effects().insert(effects));
                    } else {
                        let color = match Self::parse_color(token) {
                            Ok(Some(color)) => color,
                            Ok(None) => {
                                return Err(ParseErrorKind::UnsupportedStyle.with_pos(token_range));
                            }
                            Err(err) => return Err(err.with_pos(token_range)),
                        };
                        if !set_fields.set_fg() {
                            return Err(ParseErrorKind::DuplicateSpecifier.with_pos(token_range));
                        }
                        style = style.fg_color(Some(color));
                    }
                }
            }

            is_initial = false;
        }
        Err(self.error(ParseErrorKind::UnfinishedStyle))
    }

    const fn parse_effects(token: &[u8]) -> Option<Effects> {
        Some(match token {
            b"bold" | b"b" => Effects::BOLD,
            b"italic" | b"i" => Effects::ITALIC,
            b"underline" | b"u" | b"ul" => Effects::UNDERLINE,
            b"strikethrough" | b"strike" | b"s" => Effects::STRIKETHROUGH,
            b"dim" | b"dimmed" => Effects::DIMMED,
            b"invert" | b"inverted" | b"inv" => Effects::INVERT,
            b"blink" => Effects::BLINK,
            b"hide" | b"hidden" | b"conceal" | b"concealed" => Effects::HIDDEN,
            _ => return None,
        })
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
        if s.len() > 3 || s[0] == b'0' {
            // We disallow colors starting from `0` (e.g., `001`) to avoid ambiguity.
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

    const fn gobble_whitespace(&mut self) {
        while !self.is_eof() {
            let ch = self.current_byte();
            if ch.is_ascii_whitespace() {
                self.advance_byte();
            } else {
                break;
            }
        }
    }

    const fn gobble_punct(&mut self) -> bool {
        if !self.is_eof() && matches!(self.current_byte(), b',' | b';') {
            self.advance_byte();
            true
        } else {
            false
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
            let mut span_extended = false;
            if let Some(last_span) = spans.last_mut() {
                if is_same_style(&last_span.style, &span.style) {
                    last_span.len += span.len;
                    span_extended = true;
                }
            }

            if !span_extended && spans.push(span).is_err() {
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

impl FromStr for DynStyled {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let mut parser = RichParser::new(raw);
        let mut text = String::new();
        let mut spans = Vec::new();

        while let ops::ControlFlow::Continue((span, text_start)) = parser.step()? {
            spans.push(span);
            text.push_str(&raw[text_start..text_start + span.len]);
        }
        Ok(Self { text, spans })
    }
}

#[derive(Debug)]
pub(crate) struct RichStyle<'a>(pub(crate) &'a Style);

impl fmt::Display for RichStyle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let effects = self.0.get_effects();
        if effects.contains(Effects::BOLD) {
            write!(formatter, "bold ")?;
        }
        if effects.contains(Effects::ITALIC) {
            write!(formatter, "italic ")?;
        }
        if effects.contains(Effects::DIMMED) {
            write!(formatter, "dim ")?;
        }
        if effects.contains(Effects::UNDERLINE) {
            write!(formatter, "underline ")?;
        }
        if effects.contains(Effects::STRIKETHROUGH) {
            write!(formatter, "strike ")?;
        }
        if effects.contains(Effects::INVERT) {
            write!(formatter, "invert ")?;
        }
        if effects.contains(Effects::BLINK) {
            write!(formatter, "blink ")?;
        }
        if effects.contains(Effects::HIDDEN) {
            write!(formatter, "hidden ")?;
        }

        if let Some(color) = self.0.get_fg_color() {
            write_color(formatter, color)?;
            formatter.write_str(" ")?;
        }

        if let Some(color) = self.0.get_bg_color() {
            write!(formatter, "on ")?;
            write_color(formatter, color)?;
        }
        Ok(())
    }
}

fn write_color(formatter: &mut fmt::Formatter<'_>, color: Color) -> fmt::Result {
    match color {
        Color::Ansi(color) => write!(
            formatter,
            "{base}{bright}",
            base = ansi_color_str(color),
            bright = if color.is_bright() { "*" } else { "" }
        ),
        Color::Ansi256(Ansi256Color(idx)) => write!(formatter, "{idx}"),
        Color::Rgb(RgbColor(r, g, b)) => {
            if r % 17 == 0 && g % 17 == 0 && b % 17 == 0 {
                write!(formatter, "#{:x}{:x}{:x}", r / 17, g / 17, b / 17)
            } else {
                write!(formatter, "#{r:02x}{g:02x}{b:02x}")
            }
        }
    }
}

fn ansi_color_str(color: AnsiColor) -> &'static str {
    match color.bright(false) {
        AnsiColor::Black => "black",
        AnsiColor::Red => "red",
        AnsiColor::Green => "green",
        AnsiColor::Yellow => "yellow",
        AnsiColor::Blue => "blue",
        AnsiColor::Magenta => "magenta",
        AnsiColor::Cyan => "cyan",
        AnsiColor::White => "white",
        _ => unreachable!(),
    }
}

/// Escapes sequences of >=2 opening brackets (i.e., `[[`, `[[[` etc.) by appending `[[*]]` to each sequence
/// (a no-op style copy).
#[derive(Debug)]
pub(crate) struct EscapedText<'a>(pub(crate) &'a str);

impl fmt::Display for EscapedText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut remainder = self.0;
        while let Some(mut pos) = remainder.find("[[") {
            // Increase `pos` until it points at a non-`[` char.
            pos += 2;
            while remainder.as_bytes().get(pos).copied() == Some(b'[') {
                pos += 1;
            }

            let head;
            (head, remainder) = remainder.split_at(pos);
            write!(formatter, "{head}[[*]]")?;
        }
        write!(formatter, "{remainder}")
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
        let mut cursor = StrCursor::new("bold, ul magenta on yellow*]]");
        let style = cursor.parse_style(&Style::new()).unwrap();
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
        let mut cursor = StrCursor::new("dim i invert; blink; 42 on #c0ffee]]");
        let style = cursor.parse_style(&Style::new()).unwrap();
        let expected_style = Style::new()
            .dimmed()
            .blink()
            .invert()
            .italic()
            .fg_color(Some(RgbColor(0, 215, 135).into()))
            .bg_color(Some(RgbColor(0xc0, 0xff, 0xee).into()));
        assert_eq!(style, expected_style);
        assert!(cursor.is_eof(), "{cursor:?}");
    }

    #[test]
    fn escaping_text() {
        assert_eq!(EscapedText("test: [OK]").to_string(), "test: [OK]");

        assert_eq!(EscapedText("test: [[OK]]").to_string(), "test: [[[[*]]OK]]");

        assert_eq!(
            EscapedText("[[OK]] test :[[[").to_string(),
            "[[[[*]]OK]] test :[[[[[*]]"
        );
    }
}
