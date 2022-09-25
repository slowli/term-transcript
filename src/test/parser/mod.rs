//! SVG parsing logic.

use quick_xml::{
    events::{attributes::Attributes, Event},
    Reader as XmlReader,
};
use termcolor::WriteColor;

use std::{
    borrow::Cow,
    error::Error as StdError,
    fmt,
    io::{self, BufRead},
    mem, str,
};

#[cfg(test)]
mod tests;
mod text;

use self::text::TextReadingState;
use crate::{test::color_diff::ColorSpan, Interaction, TermOutput, Transcript, UserInput};

/// Parsed terminal output.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub(crate) plaintext: String,
    pub(crate) color_spans: Vec<ColorSpan>,
    pub(crate) html: String,
}

impl Parsed {
    /// Returns the parsed plaintext.
    pub fn plaintext(&self) -> &str {
        &self.plaintext
    }

    /// Writes the parsed text with coloring / styles applied.
    ///
    /// # Errors
    ///
    /// - Returns an I/O error should it occur when writing to `out`.
    #[doc(hidden)] // makes `termcolor` dependency public, which we want to avoid so far
    pub fn write_colorized(&self, out: &mut impl WriteColor) -> io::Result<()> {
        ColorSpan::write_colorized(&self.color_spans, out, &self.plaintext)
    }

    /// Returns the parsed HTML.
    pub fn html(&self) -> &str {
        &self.html
    }

    /// Converts this parsed fragment into text for `UserInput`. This takes into account
    /// that while the first space after prompt is inserted automatically, the further whitespace
    /// may be significant.
    fn into_input_text(self) -> String {
        if self.plaintext.starts_with(' ') {
            self.plaintext[1..].to_owned()
        } else {
            self.plaintext
        }
    }
}

impl TermOutput for Parsed {}

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
            let event = reader.read_event_into(&mut buffer)?;
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

fn parse_class(attributes: Attributes<'_>) -> Result<Cow<'_, [u8]>, ParseError> {
    let mut class = None;
    for attr in attributes {
        let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
        if attr.key.as_ref() == b"class" {
            class = Some(attr.value);
        }
    }
    Ok(class.unwrap_or(Cow::Borrowed(b"")))
}

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

#[derive(Debug, Default)]
struct UserInputState {
    text: TextReadingState,
    prompt: Option<Cow<'static, str>>,
    prompt_open_tags: Option<usize>,
}

impl UserInputState {
    /// Can prompt reading be started now?
    fn can_start_prompt(&self) -> bool {
        self.text.is_empty() && self.prompt.is_none() && self.prompt_open_tags.is_none()
    }

    fn can_end_prompt(&self) -> bool {
        self.prompt.is_none()
            && self
                .prompt_open_tags
                .map_or(false, |tags| tags + 1 == self.text.open_tags())
    }

    fn process(&mut self, event: Event<'_>) -> Result<Option<UserInput>, ParseError> {
        let mut is_prompt_end = false;
        if let Event::Start(tag) = &event {
            if self.can_start_prompt() && parse_class(tag.attributes())?.as_ref() == b"prompt" {
                // Got prompt start.
                self.prompt_open_tags = Some(self.text.open_tags());
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
            text: parsed.into_input_text(),
            prompt: self.prompt.take(),
        }))
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

impl ParserState {
    const DUMMY_INPUT: UserInput = UserInput {
        text: String::new(),
        prompt: None,
    };

    fn process(&mut self, event: Event<'_>) -> Result<Option<Interaction<Parsed>>, ParseError> {
        match self {
            Self::Initialized => {
                if let Event::Start(tag) = event {
                    if tag.name().as_ref() == b"svg" {
                        *self = Self::EncounteredSvgTag;
                    } else {
                        let tag_name = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
                        return Err(ParseError::UnexpectedRoot(tag_name));
                    }
                }
            }

            Self::EncounteredSvgTag => {
                if let Event::Start(tag) = event {
                    if tag.name().as_ref() == b"div" {
                        Self::verify_container_attrs(tag.attributes())?;
                        *self = Self::EncounteredContainer;
                    }
                }
            }

            Self::EncounteredContainer => {
                if let Event::Start(tag) = event {
                    if parse_class(tag.attributes())?.as_ref() == b"user-input" {
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
                    let class = parse_class(tag.attributes())?;
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
            let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
            match attr.key.as_ref() {
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
}
