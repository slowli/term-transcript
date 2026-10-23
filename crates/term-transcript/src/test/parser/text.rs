//! Text parsing.

use std::{borrow::Cow, fmt, mem, ops, str};

use anstyle::{AnsiColor, Color, Effects, Style};
use quick_xml::{
    escape::{EscapeError, resolve_xml_entity},
    events::{BytesStart, Event},
};
use styled_str::{StyledString, StyledStringBuilder, parse_hex_color};

use super::{ParseError, extract_base_class, parse_classes};

fn normalize_newlines(s: &str) -> Cow<'_, str> {
    if s.contains("\r\n") {
        Cow::Owned(s.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

#[derive(Debug)]
enum HardBreak {
    Active,
    JustEnded,
}

pub(super) struct TextReadingState {
    builder: StyledStringBuilder,
    open_tags: usize,
    hard_br: Option<HardBreak>,
}

impl fmt::Debug for TextReadingState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextReadingState")
            .field("builder", &self.builder)
            .finish_non_exhaustive()
    }
}

impl Default for TextReadingState {
    fn default() -> Self {
        Self {
            builder: StyledStringBuilder::default(),
            open_tags: 1,
            hard_br: None,
        }
    }
}

impl TextReadingState {
    pub(super) fn is_empty(&self) -> bool {
        self.builder.text().is_empty()
    }

    pub(super) fn open_tags(&self) -> usize {
        self.open_tags
    }

    #[cfg(test)]
    pub(super) fn plaintext_buffer(&self) -> &str {
        self.builder.text()
    }

    pub(super) fn take_plaintext(&mut self) -> String {
        mem::take(&mut self.builder).build().into_text()
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

                let unescaped_str = normalize_newlines(&text);
                let unescaped_str = if after_hard_break && unescaped_str.starts_with('\n') {
                    &unescaped_str[1..] // gobble the starting '\n' as produced by a hard break
                } else {
                    &unescaped_str
                };
                self.builder.push_text(unescaped_str);
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
                    resolve_xml_entity(&reference).ok_or_else(|| {
                        let err =
                            EscapeError::UnrecognizedEntity(position, (*reference).to_owned());
                        quick_xml::Error::from(err)
                    })?
                };
                self.builder.push_text(decoded);
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
                if extract_base_class(&classes) == "hard-br" {
                    self.hard_br = Some(HardBreak::Active);
                    return Ok(None);
                }

                if Self::is_text_span(tag_name.as_ref()) {
                    let style = Self::parse_style_from_span(&tag)?;
                    if !style.is_plain() {
                        self.builder.push_style(style);
                    }
                }
            }
            Event::End(tag) => {
                self.open_tags -= 1;
                if matches!(self.hard_br, Some(HardBreak::Active)) {
                    self.hard_br = Some(HardBreak::JustEnded);
                    return Ok(None);
                }

                if Self::is_text_span(tag.name().as_ref()) || self.open_tags == 0 {
                    self.builder.push_style(Style::new());
                }

                if self.open_tags == 0 {
                    let mut parsed = mem::take(&mut self.builder).build();
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

    fn is_text_span(tag: &str) -> bool {
        matches!(tag, "span" | "tspan" | "text")
    }

    /// Parses a style from a `span`.
    ///
    /// **NB.** Must correspond to the span creation logic in the `svg` module.
    fn parse_style_from_span(span_tag: &BytesStart) -> Result<Style, ParseError> {
        let class_attr = parse_classes(span_tag.attributes())?;
        let mut style = Style::new();
        Self::parse_color_from_classes(&mut style, &class_attr);

        let mut style_attr = Cow::Borrowed("");
        for attr in span_tag.attributes() {
            let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
            if attr.key.as_ref() == "style" {
                style_attr = attr.value;
            }
        }
        Self::parse_color_from_style(&mut style, &style_attr);

        if style.get_effects().contains(Effects::INVERT) {
            // Swap fg and bg colors back; they are swapped when writing to SVG.
            let bg_color = style.get_fg_color();
            let fg_color = style.get_bg_color();
            style = style.fg_color(fg_color).bg_color(bg_color);
        }

        Ok(style)
    }

    fn parse_color_from_classes(style: &mut Style, class_attr: &str) {
        let classes = class_attr.split(|ch: char| ch.is_ascii_whitespace());
        for class in classes {
            // Note that `class` may be empty because of multiple sequential whitespace chars.
            // This is OK for us.
            match class {
                "bold" => {
                    *style = style.bold();
                }
                "dimmed" => {
                    *style = style.dimmed();
                }
                "italic" => {
                    *style = style.italic();
                }
                "underline" => {
                    *style = style.underline();
                }
                "strike" => {
                    *style = style.strikethrough();
                }
                "blink" => {
                    *style = style.blink();
                }
                "concealed" => {
                    *style = style.hidden();
                }
                "inv" => {
                    *style = style.invert();
                }

                // Indexed foreground color candidate.
                fg if fg.starts_with("fg") => {
                    if let Some(color) = Self::parse_indexed_color(&fg[2..]) {
                        *style = style.fg_color(Some(color));
                    }
                }
                // Indexed background color candidate.
                bg if bg.starts_with("bg") => {
                    if let Some(color) = Self::parse_indexed_color(&bg[2..]) {
                        *style = style.bg_color(Some(color));
                    } else {
                        let color_str = &bg[2..];
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
    fn parse_color_from_style(style: &mut Style, css_style: &str) {
        for style_property in css_style.split(';') {
            let name_and_value: Vec<_> = style_property.splitn(2, ':').collect();
            let &[property_name, property_value] = name_and_value.as_slice() else {
                continue;
            };
            let property_name = property_name.trim();
            let property_value = property_value.trim();

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
    }

    fn parse_indexed_color(class: &str) -> Option<Color> {
        Some(match class {
            "0" => Color::Ansi(AnsiColor::Black),
            "1" => Color::Ansi(AnsiColor::Red),
            "2" => Color::Ansi(AnsiColor::Green),
            "3" => Color::Ansi(AnsiColor::Yellow),
            "4" => Color::Ansi(AnsiColor::Blue),
            "5" => Color::Ansi(AnsiColor::Magenta),
            "6" => Color::Ansi(AnsiColor::Cyan),
            "7" => Color::Ansi(AnsiColor::White),

            "8" => Color::Ansi(AnsiColor::BrightBlack),
            "9" => Color::Ansi(AnsiColor::BrightRed),
            "10" => Color::Ansi(AnsiColor::BrightGreen),
            "11" => Color::Ansi(AnsiColor::BrightYellow),
            "12" => Color::Ansi(AnsiColor::BrightBlue),
            "13" => Color::Ansi(AnsiColor::BrightMagenta),
            "14" => Color::Ansi(AnsiColor::BrightCyan),
            "15" => Color::Ansi(AnsiColor::BrightWhite),

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
            TextReadingState::parse_indexed_color("0"),
            Some(AnsiColor::Black.into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color("3"),
            Some(AnsiColor::Yellow.into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color("9"),
            Some(AnsiColor::BrightRed.into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color("10"),
            Some(AnsiColor::BrightGreen.into())
        );
        assert_eq!(
            TextReadingState::parse_indexed_color("15"),
            Some(AnsiColor::BrightWhite.into())
        );

        assert_eq!(TextReadingState::parse_indexed_color(""), None);
        assert_eq!(TextReadingState::parse_indexed_color("17"), None);
        assert_eq!(TextReadingState::parse_indexed_color("01"), None);
        assert_eq!(TextReadingState::parse_indexed_color("333"), None);
    }

    #[test]
    fn parsing_style_from_classes() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_classes(&mut style, "bold fg3 underline bg11");

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
                .bg_color(Some(AnsiColor::Yellow.into()))
        );

        let tag =
            BytesStart::from_content(r#"span class="italic inv bg5" style="color: #c0ffee;""#, 4);
        let style = TextReadingState::parse_style_from_span(&tag).unwrap();
        assert_eq!(
            style,
            Style::new()
                .italic()
                .invert()
                .fg_color(Some(AnsiColor::Magenta.into()))
                .bg_color(Some(RgbColor(0xc0, 0xff, 0xee).into()))
        );
    }

    #[test]
    fn parsing_color_from_style() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_style(&mut style, "color: #fed; background: #c0ffee");

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
        TextReadingState::parse_color_from_style(&mut style, "color: #fed; background: #c0ffee;");

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
        TextReadingState::parse_color_from_style(&mut style, "fill: #fed; stroke: #fed");

        assert_eq!(
            style.get_fg_color(),
            Some(Color::Rgb(RgbColor(0xff, 0xee, 0xdd)))
        );
        assert_eq!(style.get_bg_color(), None);
    }

    #[test]
    fn parsing_bg_color_from_svg_style() {
        let mut style = Style::default();
        TextReadingState::parse_color_from_classes(&mut style, "bold fg3 bg#d7d75f");
        assert_eq!(style.get_effects(), Effects::BOLD);
        assert_eq!(style.get_fg_color(), Some(AnsiColor::Yellow.into()));
        assert_eq!(
            style.get_bg_color(),
            Some(Color::Rgb(RgbColor(0xd7, 0xd7, 0x5f)))
        );

        let mut style = Style::default();
        TextReadingState::parse_color_from_classes(&mut style, "underline strike italic dimmed");
        assert_eq!(
            style.get_effects(),
            Effects::UNDERLINE | Effects::STRIKETHROUGH | Effects::ITALIC | Effects::DIMMED
        );
    }
}
