//! Text parsing.

use std::{borrow::Cow, fmt, mem, ops, str};

use quick_xml::{
    escape::{resolve_xml_entity, EscapeError},
    events::{BytesStart, Event},
};

use super::{extract_base_class, map_utf8_error, parse_classes, ParseError, Parsed};
use crate::{
    style::{Color, RgbColor, Style, WriteStyled},
    test::color_diff::ColorSpansWriter,
    utils::normalize_newlines,
};

#[derive(Debug)]
enum HardBreak {
    Active,
    JustEnded,
}

pub(super) struct TextReadingState {
    pub plaintext_buffer: String,
    color_spans_writer: ColorSpansWriter,
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
            color_spans_writer: ColorSpansWriter::default(),
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
    ) -> Result<Option<Parsed>, ParseError> {
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
                // Check for the hard line break <tspan> or <b>. We mustn't add its contents to the text,
                // and instead gobble the following '\n'.
                let classes = parse_classes(tag.attributes())?;
                if extract_base_class(&classes) == b"hard-br" {
                    self.hard_br = Some(HardBreak::Active);
                    return Ok(None);
                }

                if Self::is_text_span(tag_name.as_ref()) {
                    let color_spec = Self::parse_color_from_span(&tag)?;
                    if !color_spec.is_none() {
                        self.color_spans_writer
                            .write_style(&color_spec)
                            .expect("cannot set color for ANSI buffer");
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
                    self.color_spans_writer
                        .reset()
                        .expect("cannot reset color for ANSI buffer");
                }

                if self.open_tags == 0 {
                    let plaintext = mem::take(&mut self.plaintext_buffer);
                    let color_spans = mem::take(&mut self.color_spans_writer).into_inner();
                    let mut parsed = Parsed {
                        plaintext,
                        color_spans,
                    };
                    parsed.trim_ending_newline();
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
        self.color_spans_writer
            .write_text(text)
            .expect("cannot write to ANSI buffer");
    }

    /// Parses color spec from a `span`.
    ///
    /// **NB.** Must correspond to the span creation logic in the `html` module.
    fn parse_color_from_span(span_tag: &BytesStart) -> Result<Style, ParseError> {
        let class_attr = parse_classes(span_tag.attributes())?;
        let mut color_spec = Style::default();
        Self::parse_color_from_classes(&mut color_spec, &class_attr);

        let mut style = Cow::Borrowed(&[] as &[u8]);
        for attr in span_tag.attributes() {
            let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
            if attr.key.as_ref() == b"style" {
                style = attr.value;
            }
        }
        Self::parse_color_from_style(&mut color_spec, &style)?;

        Ok(color_spec)
    }

    fn parse_color_from_classes(style: &mut Style, class_attr: &[u8]) {
        let classes = class_attr.split(u8::is_ascii_whitespace);
        for class in classes {
            // Note that `class` may be empty because of multiple sequential whitespace chars.
            // This is OK for us.
            match class {
                b"bold" => {
                    style.bold = true;
                }
                b"dimmed" => {
                    style.dimmed = true;
                }
                b"italic" => {
                    style.italic = true;
                }
                b"underline" => {
                    style.underline = true;
                }

                // Indexed foreground color candidate.
                fg if fg.starts_with(b"fg") => {
                    if let Some(color) = Self::parse_indexed_color(&fg[2..]) {
                        style.fg = Some(color);
                    }
                }
                // Indexed background color candidate.
                bg if bg.starts_with(b"bg") => {
                    if let Some(color) = Self::parse_indexed_color(&bg[2..]) {
                        style.bg = Some(color);
                    } else if let Ok(color_str) = str::from_utf8(&bg[2..]) {
                        // Parse `bg#..` classes produced by the pure SVG template
                        if let Ok(color) = color_str.parse::<RgbColor>() {
                            style.bg = Some(Color::Rgb(color));
                        }
                    }
                }

                _ => { /* Ignore other classes. */ }
            }
        }
    }

    // **NB.** This parser is pretty rudimentary (e.g., does not understand comments).
    fn parse_color_from_style(color_spec: &mut Style, style: &[u8]) -> Result<(), ParseError> {
        for style_property in style.split(|&ch| ch == b';') {
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
                    if let Ok(color) = property_value.parse::<RgbColor>() {
                        color_spec.fg = Some(Color::Rgb(color));
                    }
                }
                "background" | "background-color" => {
                    if let Ok(color) = property_value.parse::<RgbColor>() {
                        color_spec.bg = Some(Color::Rgb(color));
                    }
                }
                _ => { /* Ignore other properties. */ }
            }
        }
        Ok(())
    }

    fn parse_indexed_color(class: &[u8]) -> Option<Color> {
        Some(match class {
            b"0" => Color::BLACK,
            b"1" => Color::RED,
            b"2" => Color::GREEN,
            b"3" => Color::YELLOW,
            b"4" => Color::BLUE,
            b"5" => Color::MAGENTA,
            b"6" => Color::CYAN,
            b"7" => Color::WHITE,
            b"8" | b"9" => Color::Index(class[0] - b'0'),
            b"10" | b"11" | b"12" | b"13" | b"14" | b"15" => Color::Index(10 + class[1] - b'0'),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn parsing_color_index() {
        assert_eq!(
            TextReadingState::parse_indexed_color(b"0"),
            Some(Color::BLACK)
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"3"),
            Some(Color::YELLOW)
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"9"),
            Some(Color::Index(9))
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"10"),
            Some(Color::Index(10))
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"15"),
            Some(Color::Index(15))
        );

        assert_eq!(TextReadingState::parse_indexed_color(b""), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"17"), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"01"), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"333"), None);
    }

    #[test]
    fn parsing_color_from_classes() {
        let mut color_spec = Style::default();
        TextReadingState::parse_color_from_classes(&mut color_spec, b"bold fg3 underline bg11");

        assert!(color_spec.bold, "{color_spec:?}");
        assert!(color_spec.underline, "{color_spec:?}");
        assert_eq!(color_spec.fg, Some(Color::YELLOW));
        assert_eq!(color_spec.bg, Some(Color::INTENSE_YELLOW));
    }

    #[test]
    fn parsing_color_from_style() {
        let mut color_spec = Style::default();
        TextReadingState::parse_color_from_style(
            &mut color_spec,
            b"color: #fed; background: #c0ffee",
        )
        .unwrap();

        assert_eq!(color_spec.fg, Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd))));
        assert_eq!(color_spec.bg, Some(Color::Rgb(RgbColor(0xc0, 0xff, 0xee))));
    }

    #[test]
    fn parsing_color_from_style_with_terminal_semicolon() {
        let mut color_spec = Style::default();
        TextReadingState::parse_color_from_style(
            &mut color_spec,
            b"color: #fed; background: #c0ffee;",
        )
        .unwrap();

        assert_eq!(color_spec.fg, Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd))));
        assert_eq!(color_spec.bg, Some(Color::Rgb(RgbColor(0xc0, 0xff, 0xee))));
    }

    #[test]
    fn parsing_fg_color_from_svg_style() {
        let mut color_spec = Style::default();
        TextReadingState::parse_color_from_style(&mut color_spec, b"fill: #fed; stroke: #fed")
            .unwrap();

        assert_eq!(color_spec.fg, Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd))));
        assert_eq!(color_spec.bg, None);
    }

    #[test]
    fn parsing_bg_color_from_svg_style() {
        let mut color_spec = Style::default();
        TextReadingState::parse_color_from_classes(&mut color_spec, b"bold fg3 bg#d7d75f");
        assert!(color_spec.bold);
        assert_eq!(color_spec.fg, Some(Color::YELLOW));
        assert_eq!(color_spec.bg, Some(Color::Rgb(RgbColor(0xd7, 0xd7, 0x5f))));
    }
}
