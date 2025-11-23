//! Text parsing.

use std::{borrow::Cow, fmt, io::Write, mem, ops, str};

use quick_xml::{
    escape::{resolve_xml_entity, EscapeError},
    events::{attributes::Attributes, BytesStart, Event},
};
use termcolor::{Color, ColorSpec, WriteColor};

use super::{extract_base_class, map_utf8_error, parse_classes, ParseError, Parsed};
use crate::{
    test::color_diff::ColorSpansWriter,
    utils::{normalize_newlines, RgbColor},
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
    bg_line_level: Option<usize>,
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
            bg_line_level: None,
            hard_br: None,
        }
    }
}

impl TextReadingState {
    pub fn is_empty(&self) -> bool {
        self.plaintext_buffer.is_empty()
    }

    pub fn open_tags(&self) -> usize {
        self.open_tags
    }

    fn should_ignore_text(&self) -> bool {
        self.bg_line_level.is_some() || self.hard_br.is_some()
    }

    // We only retain `<span>` tags in the HTML since they are the only ones containing color info.
    #[allow(clippy::too_many_lines)]
    pub fn process(
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
                if self.bg_line_level.is_some() {
                    return Ok(None);
                } else if self.hard_br.is_some() {
                    return Err(ParseError::InvalidHardBreak);
                }

                let tag_name = tag.name();
                let is_tspan = tag_name.as_ref() == b"tspan";
                if is_tspan && Self::is_bg_line(tag.attributes())? {
                    self.bg_line_level = Some(self.open_tags);
                    return Ok(None);
                } else if is_tspan {
                    // Check for the hard line break <tspan>. We mustn't add its contents to the text,
                    // and instead gobble the following '\n'.
                    let classes = parse_classes(tag.attributes())?;
                    if extract_base_class(&classes) == b"hard-br" {
                        self.hard_br = Some(HardBreak::Active);
                        return Ok(None);
                    }
                }

                if tag_name.as_ref() == b"span" || is_tspan {
                    let color_spec = Self::parse_color_from_span(&tag)?;
                    if !color_spec.is_none() {
                        self.color_spans_writer
                            .set_color(&color_spec)
                            .expect("cannot set color for ANSI buffer");
                    }
                }
            }
            Event::End(tag) => {
                self.open_tags -= 1;
                if let Some(level) = self.bg_line_level {
                    debug_assert!(level <= self.open_tags);
                    if self.open_tags == level {
                        self.bg_line_level = None;
                    }
                    return Ok(None);
                } else if matches!(self.hard_br, Some(HardBreak::Active)) {
                    self.hard_br = Some(HardBreak::JustEnded);
                    return Ok(None);
                }

                let tag_name = tag.name();
                let is_tspan = tag_name.as_ref() == b"tspan";
                if tag.name().as_ref() == b"span" || is_tspan {
                    // FIXME: check embedded color specs (should never be produced).
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

    fn push_text(&mut self, text: &str) {
        self.plaintext_buffer.push_str(text);
        self.color_spans_writer
            .write_all(text.as_bytes())
            .expect("cannot write to ANSI buffer");
    }

    fn is_bg_line(attrs: Attributes<'_>) -> Result<bool, ParseError> {
        let classes = parse_classes(attrs)?;
        Ok(extract_base_class(&classes) == b"output-bg")
    }

    /// Parses color spec from a `span`.
    ///
    /// **NB.** Must correspond to the span creation logic in the `html` module.
    fn parse_color_from_span(span_tag: &BytesStart) -> Result<ColorSpec, ParseError> {
        let class_attr = parse_classes(span_tag.attributes())?;
        let mut color_spec = ColorSpec::new();
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

    fn parse_color_from_classes(color_spec: &mut ColorSpec, class_attr: &[u8]) {
        let classes = class_attr.split(u8::is_ascii_whitespace);
        for class in classes {
            // Note that `class` may be empty because of multiple sequential whitespace chars.
            // This is OK for us.
            match class {
                b"bold" => {
                    color_spec.set_bold(true);
                }
                b"dimmed" => {
                    color_spec.set_dimmed(true);
                }
                b"italic" => {
                    color_spec.set_italic(true);
                }
                b"underline" => {
                    color_spec.set_underline(true);
                }

                // Indexed foreground color candidate.
                fg if fg.starts_with(b"fg") => {
                    if let Some(color) = Self::parse_indexed_color(&fg[2..]) {
                        color_spec.set_fg(Some(color));
                    }
                }
                // Indexed background color candidate.
                bg if bg.starts_with(b"bg") => {
                    if let Some(color) = Self::parse_indexed_color(&bg[2..]) {
                        color_spec.set_bg(Some(color));
                    } else if let Ok(color_str) = str::from_utf8(&bg[2..]) {
                        // Parse `bg#..` classes produced by the pure SVG template
                        if let Ok(color) = color_str.parse::<RgbColor>() {
                            color_spec.set_bg(Some(color.into_ansi_color()));
                        }
                    }
                }

                _ => { /* Ignore other classes. */ }
            }
        }
    }

    // **NB.** This parser is pretty rudimentary (e.g., does not understand comments).
    fn parse_color_from_style(color_spec: &mut ColorSpec, style: &[u8]) -> Result<(), ParseError> {
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
                        color_spec.set_fg(Some(color.into_ansi_color()));
                    }
                }
                "background" | "background-color" => {
                    if let Ok(color) = property_value.parse::<RgbColor>() {
                        color_spec.set_bg(Some(color.into_ansi_color()));
                    }
                }
                _ => { /* Ignore other properties. */ }
            }
        }
        Ok(())
    }

    fn parse_indexed_color(class: &[u8]) -> Option<Color> {
        Some(match class {
            b"0" => Color::Black,
            b"1" => Color::Red,
            b"2" => Color::Green,
            b"3" => Color::Yellow,
            b"4" => Color::Blue,
            b"5" => Color::Magenta,
            b"6" => Color::Cyan,
            b"7" => Color::White,
            b"8" | b"9" => Color::Ansi256(class[0] - b'0'),
            b"10" | b"11" | b"12" | b"13" | b"14" | b"15" => Color::Ansi256(10 + class[1] - b'0'),
            _ => return None,
        })
    }
}

impl RgbColor {
    fn into_ansi_color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_color_index() {
        assert_eq!(
            TextReadingState::parse_indexed_color(b"0"),
            Some(Color::Black)
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"3"),
            Some(Color::Yellow)
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"9"),
            Some(Color::Ansi256(9))
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"10"),
            Some(Color::Ansi256(10))
        );
        assert_eq!(
            TextReadingState::parse_indexed_color(b"15"),
            Some(Color::Ansi256(15))
        );

        assert_eq!(TextReadingState::parse_indexed_color(b""), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"17"), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"01"), None);
        assert_eq!(TextReadingState::parse_indexed_color(b"333"), None);
    }

    #[test]
    fn parsing_color_from_classes() {
        let mut color_spec = ColorSpec::new();
        TextReadingState::parse_color_from_classes(&mut color_spec, b"bold fg3 underline bg11");

        assert!(color_spec.bold(), "{color_spec:?}");
        assert!(color_spec.underline(), "{color_spec:?}");
        assert_eq!(color_spec.fg(), Some(&Color::Yellow));
        assert_eq!(color_spec.bg(), Some(&Color::Ansi256(11)));
    }

    #[test]
    fn parsing_color_from_style() {
        let mut color_spec = ColorSpec::new();
        TextReadingState::parse_color_from_style(
            &mut color_spec,
            b"color: #fed; background: #c0ffee",
        )
        .unwrap();

        assert_eq!(color_spec.fg(), Some(&Color::Rgb(0xff, 0xee, 0xdd)));
        assert_eq!(color_spec.bg(), Some(&Color::Rgb(0xc0, 0xff, 0xee)));
    }

    #[test]
    fn parsing_color_from_style_with_terminal_semicolon() {
        let mut color_spec = ColorSpec::new();
        TextReadingState::parse_color_from_style(
            &mut color_spec,
            b"color: #fed; background: #c0ffee;",
        )
        .unwrap();

        assert_eq!(color_spec.fg(), Some(&Color::Rgb(0xff, 0xee, 0xdd)));
        assert_eq!(color_spec.bg(), Some(&Color::Rgb(0xc0, 0xff, 0xee)));
    }

    #[test]
    fn parsing_fg_color_from_svg_style() {
        let mut color_spec = ColorSpec::new();
        TextReadingState::parse_color_from_style(&mut color_spec, b"fill: #fed; stroke: #fed")
            .unwrap();

        assert_eq!(color_spec.fg(), Some(&Color::Rgb(0xff, 0xee, 0xdd)));
        assert_eq!(color_spec.bg(), None);
    }

    #[test]
    fn parsing_bg_color_from_svg_style() {
        let mut color_spec = ColorSpec::new();
        TextReadingState::parse_color_from_classes(&mut color_spec, b"bold fg3 bg#d7d75f");
        assert!(color_spec.bold());
        assert_eq!(color_spec.fg(), Some(&Color::Yellow));
        assert_eq!(color_spec.bg(), Some(&Color::Rgb(0xd7, 0xd7, 0x5f)));
    }
}
