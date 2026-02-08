//! Text parsing.

use std::{borrow::Cow, fmt, mem, ops, str};

use anstyle::{Ansi256Color, AnsiColor, Color, Effects, Style};
use quick_xml::{
    escape::{EscapeError, resolve_xml_entity},
    events::{BytesStart, Event},
};
use term_style::{StyledSpan, StyledString, parse_hex_color};

use super::{ParseError, extract_base_class, map_utf8_error, parse_classes};
use crate::utils::normalize_newlines;

#[derive(Debug)]
enum HardBreak {
    Active,
    JustEnded,
}

pub(super) struct TextReadingState {
    pub plaintext_buffer: String,
    style_spans: Vec<StyledSpan>,
    open_tags: usize,
    hard_br: Option<HardBreak>,
}

impl fmt::Debug for TextReadingState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextReadingState")
            .field("plaintext_buffer", &self.plaintext_buffer)
            .finish_non_exhaustive()
    }
}

impl Default for TextReadingState {
    fn default() -> Self {
        Self {
            style_spans: Vec::new(),
            plaintext_buffer: String::new(),
            open_tags: 1,
            hard_br: None,
        }
    }
}

impl TextReadingState {
    pub(super) fn is_empty(&self) -> bool {
        self.plaintext_buffer.is_empty()
    }

    pub(super) fn open_tags(&self) -> usize {
        self.open_tags
    }

    fn should_ignore_text(&self) -> bool {
        self.hard_br.is_some()
    }

    // We only retain `<span>` tags in the HTML since they are the only ones containing color info.
    #[allow(clippy::too_many_lines)]
    pub(super) fn process(
        &mut self,
        event: Event<'_>,
        position: ops::Range<usize>,
    ) -> Result<Option<StyledString>, ParseError> {
        let after_hard_break = matches!(self.hard_br, Some(HardBreak::JustEnded));
        if after_hard_break
            && matches!(
                &event,
                Event::Text(_) | Event::GeneralRef(_) | Event::Start(_)
            )
        {
            self.hard_br = None;
        }

        match event {
            Event::Text(text) => {
                if self.should_ignore_text() {
                    return Ok(None);
                }

                let unescaped_str = text.decode().map_err(quick_xml::Error::from)?;
                let unescaped_str = normalize_newlines(&unescaped_str);
                let unescaped_str = if after_hard_break && unescaped_str.starts_with('\n') {
                    &unescaped_str[1..] // gobble the starting '\n' as produced by a hard break
                } else {
                    &unescaped_str
                };
                self.push_text(unescaped_str);
            }
            Event::GeneralRef(reference) => {
                if self.should_ignore_text() {
                    return Ok(None);
                }

                let maybe_char = reference.resolve_char_ref()?;
                let mut char_buffer = [0_u8; 4];
                let decoded = if let Some(c) = maybe_char {
                    c.encode_utf8(&mut char_buffer)
                } else {
                    let decoded = reference.decode().map_err(quick_xml::Error::from)?;
                    resolve_xml_entity(&decoded).ok_or_else(|| {
                        let err = EscapeError::UnrecognizedEntity(position, decoded.into_owned());
                        quick_xml::Error::from(err)
                    })?
                };
                self.push_text(decoded);
            }
            Event::Start(tag) => {
                self.open_tags += 1;
                if self.hard_br.is_some() {
                    return Err(ParseError::InvalidHardBreak);
                }

                let tag_name = tag.name();
                // Check for the hard line break <text> or <b>. We mustn't add its contents to the text,
                // and instead gobble the following '\n'.
                let classes = parse_classes(tag.attributes())?;
                if extract_base_class(&classes) == b"hard-br" {
                    self.hard_br = Some(HardBreak::Active);
                    return Ok(None);
                }

                if Self::is_text_span(tag_name.as_ref()) {
                    let style = Self::parse_style_from_span(&tag)?;
                    if !style.is_plain() {
                        self.style_spans.push(StyledSpan { style, len: 0 });
                    }
                }
            }
            Event::End(tag) => {
                self.open_tags -= 1;
                if matches!(self.hard_br, Some(HardBreak::Active)) {
                    self.hard_br = Some(HardBreak::JustEnded);
                    return Ok(None);
                }

                if Self::is_text_span(tag.name().as_ref()) {
                    self.style_spans.push(StyledSpan {
                        style: Style::new(),
                        len: 0,
                    });
                }

                if self.open_tags == 0 {
                    let plaintext = mem::take(&mut self.plaintext_buffer);
                    let styled_spans = mem::take(&mut self.style_spans);
                    let mut parsed = StyledString::from_parts(plaintext, styled_spans);
                    if parsed.text().ends_with('\n') {
                        parsed.pop();
                    }
                    return Ok(Some(parsed));
                }
            }
            _ => { /* Do nothing */ }
        }
        Ok(None)
    }

    fn is_text_span(tag: &[u8]) -> bool {
        matches!(tag, b"span" | b"tspan" | b"text")
    }

    fn push_text(&mut self, text: &str) {
        self.plaintext_buffer.push_str(text);
        if let Some(last_span) = self.style_spans.last_mut() {
            last_span.len += text.len();
        }
    }

    /// Parses a style from a `span`.
    ///
    /// **NB.** Must correspond to the span creation logic in the `svg` module.
    fn parse_style_from_span(span_tag: &BytesStart) -> Result<Style, ParseError> {
        let class_attr = parse_classes(span_tag.attributes())?;
        let mut style = Style::new();
        Self::parse_color_from_classes(&mut style, &class_attr);

        let mut style_attr = Cow::Borrowed(&[] as &[u8]);
        for attr in span_tag.attributes() {
            let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
            if attr.key.as_ref() == b"style" {
                style_attr = attr.value;
            }
        }
        Self::parse_color_from_style(&mut style, &style_attr)?;

        if style.get_effects().contains(Effects::INVERT) {
            // Swap fg and bg colors back; they are swapped when writing to SVG.
            let bg_color = style.get_fg_color();
            let fg_color = style.get_bg_color();
            style = style.fg_color(fg_color).bg_color(bg_color);
        }

        Ok(style)
    }

    fn parse_color_from_classes(style: &mut Style, class_attr: &[u8]) {
        let classes = class_attr.split(u8::is_ascii_whitespace);
        for class in classes {
            // Note that `class` may be empty because of multiple sequential whitespace chars.
            // This is OK for us.
            match class {
                b"bold" => {
                    *style = style.bold();
                }
                b"dimmed" => {
                    *style = style.dimmed();
                }
                b"italic" => {
                    *style = style.italic();
                }
                b"underline" => {
                    *style = style.underline();
                }
                b"strike" => {
                    *style = style.strikethrough();
                }
                b"blink" => {
                    *style = style.blink();
                }
                b"concealed" => {
                    *style = style.hidden();
                }
                b"inv" => {
                    *style = style.invert();
                }

                // Indexed foreground color candidate.
                fg if fg.starts_with(b"fg") => {
                    if let Some(color) = Self::parse_indexed_color(&fg[2..]) {
                        *style = style.fg_color(Some(color));
                    }
                }
                // Indexed background color candidate.
                bg if bg.starts_with(b"bg") => {
                    if let Some(color) = Self::parse_indexed_color(&bg[2..]) {
                        *style = style.bg_color(Some(color));
                    } else if let Ok(color_str) = str::from_utf8(&bg[2..]) {
                        // Parse `bg#..` classes produced by the pure SVG template
                        if let Ok(color) = parse_hex_color(color_str.as_bytes()) {
                            *style = style.bg_color(Some(color.into()));
                        }
                    }
                }

                _ => { /* Ignore other classes. */ }
            }
        }
    }

    // **NB.** This parser is pretty rudimentary (e.g., does not understand comments).
    fn parse_color_from_style(style: &mut Style, css_style: &[u8]) -> Result<(), ParseError> {
        for style_property in css_style.split(|&ch| ch == b';') {
            let name_and_value: Vec<_> = style_property.splitn(2, |&ch| ch == b':').collect();
            let [property_name, property_value] = name_and_value.as_slice() else {
                continue;
            };

            let property_name = str::from_utf8(property_name)
                .map_err(map_utf8_error)?
                .trim();
            let property_value = str::from_utf8(property_value)
                .map_err(map_utf8_error)?
                .trim();

            match property_name {
                "color" | "fill" => {
                    if let Ok(color) = parse_hex_color(property_value.as_bytes()) {
                        *style = style.fg_color(Some(color.into()));
                    }
                }
                "background" | "background-color" => {
                    if let Ok(color) = parse_hex_color(property_value.as_bytes()) {
                        *style = style.bg_color(Some(color.into()));
                    }
                }
                _ => { /* Ignore other properties. */ }
            }
        }
        Ok(())
    }

    fn parse_indexed_color(class: &[u8]) -> Option<Color> {
        Some(match class {
            b"0" => Color::Ansi(AnsiColor::Black),
            b"1" => Color::Ansi(AnsiColor::Red),
            b"2" => Color::Ansi(AnsiColor::Green),
            b"3" => Color::Ansi(AnsiColor::Yellow),
            b"4" => Color::Ansi(AnsiColor::Blue),
            b"5" => Color::Ansi(AnsiColor::Magenta),
            b"6" => Color::Ansi(AnsiColor::Cyan),
            b"7" => Color::Ansi(AnsiColor::White),
            b"8" | b"9" => Color::Ansi256(Ansi256Color(class[0] - b'0')),
            b"10" | b"11" | b"12" | b"13" | b"14" | b"15" => {
                Color::Ansi256(Ansi256Color(10 + class[1] - b'0'))
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use anstyle::RgbColor;

    use super::*;

    #[test]
    fn parsing_color_index() {
        assert_eq!(
            TextReadingState::parse_indexed_color(b"0"),
            Some(AnsiColor::Black.into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"3"),
            Some(AnsiColor::Yellow.into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"9"),
            Some(Ansi256Color(9).into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"10"),
            Some(Ansi256Color(10).into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"15"),
            Some(Ansi256Color(15).into())
        );

        assert_eq!(TextReadingState::parse_indexed_color(b""), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"17"), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"01"), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"333"), None);
    }

    #[test]
    fn parsing_style_from_classes() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_classes(&mut style, b"bold fg3 underline bg11");

        assert_eq!(style.get_effects(), Effects::BOLD | Effects::UNDERLINE);
        assert_eq!(style.get_fg_color(), Some(AnsiColor::Yellow.into()));
        assert_eq!(style.get_bg_color(), Some(AnsiColor::BrightYellow.into()));
    }

    #[test]
    fn parsing_inverted_style_from_classes() {
        let tag = BytesStart::from_content(r#"span class="bold inv fg3""#, 4);
        let style = TextReadingState::parse_style_from_span(&tag).unwrap();
        assert_eq!(
            style,
            Style::new()
                .bold()
                .invert()
                .bg_color(Some(Ansi256Color(3).into()))
        );

        let tag =
            BytesStart::from_content(r#"span class="italic inv bg5" style="color: #c0ffee;""#, 4);
        let style = TextReadingState::parse_style_from_span(&tag).unwrap();
        assert_eq!(
            style,
            Style::new()
                .italic()
                .invert()
                .fg_color(Some(Ansi256Color(5).into()))
                .bg_color(Some(RgbColor(0xc0, 0xff, 0xee).into()))
        );
    }

    #[test]
    fn parsing_color_from_style() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_style(&mut style, b"color: #fed; background: #c0ffee")
            .unwrap();

        assert_eq!(
            style.get_fg_color(),
            Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd)))
        );
        assert_eq!(
            style.get_bg_color(),
            Some(Color::Rgb(RgbColor(0xc0, 0xff, 0xee)))
        );
    }

    #[test]
    fn parsing_color_from_style_with_terminal_semicolon() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_style(&mut style, b"color: #fed; background: #c0ffee;")
            .unwrap();

        assert_eq!(
            style.get_fg_color(),
            Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd)))
        );
        assert_eq!(
            style.get_bg_color(),
            Some(Color::Rgb(RgbColor(0xc0, 0xff, 0xee)))
        );
    }

    #[test]
    fn parsing_fg_color_from_svg_style() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_style(&mut style, b"fill: #fed; stroke: #fed").unwrap();

        assert_eq!(
            style.get_fg_color(),
            Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd)))
        );
        assert_eq!(style.get_bg_color(), None);
    }

    #[test]
    fn parsing_bg_color_from_svg_style() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_classes(&mut style, b"bold fg3 bg#d7d75f");
        assert_eq!(style.get_effects(), Effects::BOLD);
        assert_eq!(style.get_fg_color(), Some(AnsiColor::Yellow.into()));
        assert_eq!(
            style.get_bg_color(),
            Some(Color::Rgb(RgbColor(0xd7, 0xd7, 0x5f)))
        );

        let mut style = Style::default();
        TextReadingState::parse_color_from_classes(&mut style, b"underline strike italic dimmed");
        assert_eq!(
            style.get_effects(),
            Effects::UNDERLINE | Effects::STRIKETHROUGH | Effects::ITALIC | Effects::DIMMED
        );
    }
}
