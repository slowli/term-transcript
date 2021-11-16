//! SVG parsing logic.

use quick_xml::{
    events::{attributes::Attributes, BytesStart, Event},
    Reader as XmlReader,
};
use termcolor::{Color, ColorSpec, WriteColor};

use std::{
    borrow::Cow,
    error::Error as StdError,
    fmt,
    io::{self, BufRead, Write},
    mem, str,
};

use crate::{
    test::color_diff::{ColorSpan, ColorSpansWriter},
    utils::{normalize_newlines, RgbColor},
    Interaction, TermOutput, Transcript, UserInput,
};

#[cfg(test)]
mod tests;

/// Parsed terminal output.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub(crate) plaintext: String,
    pub(crate) color_spans: Vec<ColorSpan>,
    pub(crate) html: String,
}

impl Parsed {
    /// Gets the parsed plaintext.
    pub fn plaintext(&self) -> &str {
        &self.plaintext
    }

    /// Writes the parsed text with coloring / styles applied.
    ///
    /// # Errors
    ///
    /// - Returns an I/O error should it occur when writing to `out`.
    pub fn write_colorized(&self, out: &mut impl WriteColor) -> io::Result<()> {
        ColorSpan::write_colorized(&self.color_spans, out, &self.plaintext)
    }

    /// Gets the parsed HTML.
    pub fn html(&self) -> &str {
        &self.html
    }
}

impl TermOutput for Parsed {}

/// Errors that can occur during parsing SVG transcripts.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Unexpected root XML tag; must be `<svg>`.
    UnexpectedRoot(String),
    /// Invalid transcript container.
    InvalidContainer,
    /// Unexpected end of file.
    UnexpectedEof,
    /// Error parsing XML.
    Xml(quick_xml::Error),
}

impl From<quick_xml::Error> for ParseError {
    fn from(err: quick_xml::Error) -> Self {
        Self::Xml(err)
    }
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> Self {
        Self::Xml(quick_xml::Error::Io(err))
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedRoot(tag_name) => write!(
                formatter,
                "Unexpected root XML tag: <{}>; expected <svg>",
                tag_name
            ),
            Self::InvalidContainer => formatter.write_str("Invalid transcript container"),
            Self::UnexpectedEof => formatter.write_str("Unexpected EOF"),
            Self::Xml(err) => write!(formatter, "Error parsing XML: {}", err),
        }
    }
}

impl StdError for ParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Xml(err) => Some(err),
            _ => None,
        }
    }
}

/// States of the FSM for parsing SVGs.
#[derive(Debug)]
enum ParserState {
    /// Initial state.
    Initialized,
    /// Encountered `<svg>` tag; searching for `<div class="container">`.
    EncounteredSvgTag,
    /// Encountered `<div class="container">`; searching for `<div class="user-input">`.
    EncounteredContainer,
    /// Reading user input (`<div class="user-input">` contents).
    ReadingUserInput(UserInputState),
    /// Finished reading user input; searching for `<div class="term-output">`.
    EncounteredUserInput(UserInput),
    /// Reading terminal output (`<div class="term-output">` contents).
    ReadingTermOutput(UserInput, TextReadingState),
}

#[derive(Debug)]
struct TextReadingState {
    html_buffer: String,
    color_spans_writer: ColorSpansWriter,
    plaintext_buffer: String,
    open_tags: usize,
}

impl Default for TextReadingState {
    fn default() -> Self {
        Self {
            html_buffer: String::new(),
            color_spans_writer: ColorSpansWriter::default(),
            plaintext_buffer: String::new(),
            open_tags: 1,
        }
    }
}

impl TextReadingState {
    // We only retain `<span>` tags in the HTML since they are the only ones containing color info.
    fn process(&mut self, event: Event<'_>) -> Result<Option<Parsed>, ParseError> {
        match event {
            Event::Text(text) => {
                let unescaped = text.unescaped()?;
                let unescaped_str = str::from_utf8(&unescaped).map_err(quick_xml::Error::Utf8)?;
                let unescaped_str = normalize_newlines(unescaped_str);

                self.html_buffer.push_str(&unescaped_str);
                self.plaintext_buffer.push_str(&unescaped_str);
                self.color_spans_writer
                    .write_all(unescaped_str.as_bytes())
                    .expect("cannot write to ANSI buffer");
            }
            Event::Start(tag) => {
                self.open_tags += 1;
                if tag.name() == b"span" {
                    self.html_buffer.push('<');
                    let tag_str = str::from_utf8(&tag).map_err(quick_xml::Error::Utf8)?;
                    self.html_buffer.push_str(tag_str);
                    self.html_buffer.push('>');

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

                if tag.name() == b"span" {
                    self.html_buffer.push_str("</span>");

                    // FIXME: check embedded color specs (should never be produced).
                    self.color_spans_writer
                        .reset()
                        .expect("cannot reset color for ANSI buffer");
                }

                if self.open_tags == 0 {
                    let html = mem::take(&mut self.html_buffer);
                    let plaintext = mem::take(&mut self.plaintext_buffer);
                    let color_spans = mem::take(&mut self.color_spans_writer).into_inner();
                    return Ok(Some(Parsed {
                        plaintext,
                        color_spans,
                        html,
                    }));
                }
            }
            _ => { /* Do nothing */ }
        }
        Ok(None)
    }

    /// Parses color spec from a `span`.
    ///
    /// **NB.** Must correspond to the span creation logic in the `html` module.
    fn parse_color_from_span(span_tag: &BytesStart) -> Result<ColorSpec, ParseError> {
        let class_attr = ParserState::get_class(span_tag.attributes())?;
        let mut color_spec = ColorSpec::new();
        Self::parse_color_from_classes(&mut color_spec, &class_attr);

        let mut style = Cow::Borrowed(&[] as &[u8]);
        for attr in span_tag.attributes() {
            let attr = attr?;
            if attr.key == b"style" {
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
            let (property_name, property_value) = match name_and_value.as_slice() {
                [name, value] => (name, value),
                _ => continue,
            };

            let property_name = str::from_utf8(property_name)
                .map_err(quick_xml::Error::Utf8)?
                .trim();
            let property_value = str::from_utf8(property_value)
                .map_err(quick_xml::Error::Utf8)?
                .trim();

            match property_name {
                "color" => {
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

#[derive(Debug, Default)]
struct UserInputState {
    text: TextReadingState,
    prompt: Option<Cow<'static, str>>,
    prompt_open_tags: Option<usize>,
}

impl UserInputState {
    /// Can prompt reading be started now?
    fn can_start_prompt(&self) -> bool {
        self.text.plaintext_buffer.is_empty()
            && self.prompt.is_none()
            && self.prompt_open_tags.is_none()
    }

    fn can_end_prompt(&self) -> bool {
        self.prompt.is_none()
            && self
                .prompt_open_tags
                .map_or(false, |tags| tags + 1 == self.text.open_tags)
    }

    fn process(&mut self, event: Event<'_>) -> Result<Option<UserInput>, ParseError> {
        let mut is_prompt_end = false;
        if let Event::Start(tag) = &event {
            if self.can_start_prompt()
                && ParserState::get_class(tag.attributes())?.as_ref() == b"prompt"
            {
                // Got prompt start.
                self.prompt_open_tags = Some(self.text.open_tags);
            }
        } else if let Event::End(_) = &event {
            if self.can_end_prompt() {
                is_prompt_end = true;
            }
        }

        let maybe_parsed = self.text.process(event)?;
        if is_prompt_end {
            if let Some(parsed) = maybe_parsed {
                // Special case: user input consists of the prompt only.
                return Ok(Some(UserInput {
                    text: String::new(),
                    prompt: Some(UserInput::intern_prompt(parsed.plaintext)),
                }));
            }
            let text = mem::take(&mut self.text.plaintext_buffer);
            self.prompt = Some(UserInput::intern_prompt(text));
        }

        Ok(maybe_parsed.map(|parsed| UserInput {
            text: parsed.plaintext.trim_start().to_owned(),
            prompt: self.prompt.take(),
        }))
    }
}

impl ParserState {
    const DUMMY_INPUT: UserInput = UserInput {
        text: String::new(),
        prompt: None,
    };

    fn process(&mut self, event: Event<'_>) -> Result<Option<Interaction<Parsed>>, ParseError> {
        match self {
            Self::Initialized => {
                if let Event::Start(tag) = event {
                    if tag.name() == b"svg" {
                        *self = Self::EncounteredSvgTag;
                    } else {
                        let tag_name = String::from_utf8_lossy(tag.name()).into_owned();
                        return Err(ParseError::UnexpectedRoot(tag_name));
                    }
                }
            }

            Self::EncounteredSvgTag => {
                if let Event::Start(tag) = event {
                    if tag.name() == b"div" {
                        Self::verify_container_attrs(tag.attributes())?;
                        *self = Self::EncounteredContainer;
                    }
                }
            }

            Self::EncounteredContainer => {
                if let Event::Start(tag) = event {
                    if Self::get_class(tag.attributes())?.as_ref() == b"user-input" {
                        *self = Self::ReadingUserInput(UserInputState::default());
                    }
                }
            }

            Self::ReadingUserInput(state) => {
                if let Some(user_input) = state.process(event)? {
                    *self = Self::EncounteredUserInput(user_input);
                }
            }

            Self::EncounteredUserInput(user_input) => {
                if let Event::Start(tag) = event {
                    let class = Self::get_class(tag.attributes())?;
                    if class.as_ref() == b"term-output" {
                        let user_input = mem::replace(user_input, Self::DUMMY_INPUT);
                        *self = Self::ReadingTermOutput(user_input, TextReadingState::default());
                    } else if class.as_ref() == b"user-input" {
                        let user_input = mem::replace(user_input, Self::DUMMY_INPUT);
                        *self = Self::ReadingUserInput(UserInputState::default());

                        return Ok(Some(Interaction {
                            input: user_input,
                            output: Parsed::default(),
                        }));
                    }
                }
            }

            Self::ReadingTermOutput(user_input, text_state) => {
                if let Some(term_output) = text_state.process(event)? {
                    let user_input = mem::replace(user_input, Self::DUMMY_INPUT);
                    *self = Self::EncounteredContainer;

                    return Ok(Some(Interaction {
                        input: user_input,
                        output: term_output,
                    }));
                }
            }
        }
        Ok(None)
    }

    fn verify_container_attrs(attributes: Attributes<'_>) -> Result<(), ParseError> {
        const HTML_NS: &[u8] = b"http://www.w3.org/1999/xhtml";

        let mut has_ns_attribute = false;
        let mut has_class_attribute = false;

        for attr in attributes {
            let attr = attr?;
            match attr.key {
                b"xmlns" => {
                    if attr.value.as_ref() != HTML_NS {
                        return Err(ParseError::InvalidContainer);
                    }
                    has_ns_attribute = true;
                }
                b"class" => {
                    if attr.value.as_ref() != b"container" {
                        return Err(ParseError::InvalidContainer);
                    }
                    has_class_attribute = true;
                }
                _ => { /* Do nothing. */ }
            }
        }

        if has_ns_attribute && has_class_attribute {
            Ok(())
        } else {
            Err(ParseError::InvalidContainer)
        }
    }

    fn get_class(attributes: Attributes<'_>) -> Result<Cow<'_, [u8]>, ParseError> {
        let mut class = None;
        for attr in attributes {
            let attr = attr?;
            if attr.key == b"class" {
                class = Some(attr.value);
            }
        }
        Ok(class.unwrap_or(Cow::Borrowed(b"")))
    }
}

impl Transcript<Parsed> {
    /// Parses a transcript from the provided `reader`, which should point to an SVG XML tree
    /// produced by [`Template::render()`] (possibly within a larger document).
    ///
    /// # Errors
    ///
    /// - Returns an error if the input cannot be parsed, usually because it was not produced
    ///   by `Template::render()`.
    ///
    /// [`Template::render()`]: crate::svg::Template::render()
    pub fn from_svg<R: BufRead>(reader: R) -> Result<Self, ParseError> {
        let mut reader = XmlReader::from_reader(reader);
        let mut buffer = vec![];
        let mut state = ParserState::Initialized;
        let mut transcript = Self::new();
        let mut open_tags = 0;

        loop {
            let event = reader.read_event(&mut buffer)?;
            match &event {
                Event::Start(_) => {
                    open_tags += 1;
                }
                Event::End(_) => {
                    open_tags -= 1;
                    if open_tags == 0 {
                        break;
                    }
                }
                Event::Eof => break,
                _ => { /* Do nothing. */ }
            }

            if let Some(interaction) = state.process(event)? {
                transcript.interactions.push(interaction);
            }
        }

        if let ParserState::EncounteredContainer = state {
            Ok(transcript)
        } else {
            Err(ParseError::UnexpectedEof)
        }
    }
}
