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
    mem,
    num::ParseIntError,
    str,
};

#[cfg(test)]
mod tests;
mod text;

use self::text::TextReadingState;
use crate::{
    test::color_diff::ColorSpan, ExitStatus, Interaction, TermOutput, Transcript, UserInput,
};

/// Parsed terminal output.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub(crate) plaintext: String,
    pub(crate) color_spans: Vec<ColorSpan>,
    pub(crate) html: String,
}

impl Parsed {
    const DEFAULT: Self = Self {
        plaintext: String::new(),
        color_spans: Vec::new(),
        html: String::new(),
    };

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
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, err))]
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
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    ?interaction.input,
                    interaction.output = ?interaction.output.plaintext,
                    ?interaction.exit_status,
                    "parsed interaction"
                );
                transcript.interactions.push(interaction);
            }
        }

        match state {
            ParserState::EncounteredContainer => Ok(transcript),
            ParserState::EncounteredUserInput(interaction) => {
                transcript.interactions.push(interaction);
                Ok(transcript)
            }
            _ => Err(ParseError::UnexpectedEof),
        }
    }
}

fn parse_classes(attributes: Attributes<'_>) -> Result<Cow<'_, [u8]>, ParseError> {
    let mut class = None;
    for attr in attributes {
        let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
        if attr.key.as_ref() == b"class" {
            class = Some(attr.value);
        }
    }
    Ok(class.unwrap_or(Cow::Borrowed(b"")))
}

fn extract_base_class(classes: &[u8]) -> &[u8] {
    let space_idx = classes.iter().position(|&ch| ch == b' ');
    space_idx.map_or(classes.as_ref(), |idx| &classes[..idx])
}

fn parse_exit_status(attributes: Attributes<'_>) -> Result<Option<ExitStatus>, ParseError> {
    let mut exit_status = None;
    for attr in attributes {
        let attr = attr.map_err(quick_xml::Error::InvalidAttr)?;
        if attr.key.as_ref() == b"data-exit-status" {
            let status = str::from_utf8(&attr.value).map_err(|err| ParseError::Xml(err.into()))?;
            let status = status.parse().map_err(ParseError::InvalidExitStatus)?;
            exit_status = Some(ExitStatus(status));
        }
    }
    Ok(exit_status)
}

/// Errors that can occur during parsing SVG transcripts.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Unexpected root XML tag; must be `<svg>`.
    UnexpectedRoot(String),
    /// Invalid transcript container.
    InvalidContainer,
    /// Invalid recorded exit status of an executed command.
    InvalidExitStatus(ParseIntError),
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
        Self::Xml(err.into())
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedRoot(tag_name) => write!(
                formatter,
                "unexpected root XML tag: <{tag_name}>; expected <svg>"
            ),
            Self::InvalidContainer => formatter.write_str("invalid transcript container"),
            Self::InvalidExitStatus(err) => write!(formatter, "invalid exit status: {err}"),
            Self::UnexpectedEof => formatter.write_str("unexpected EOF"),
            Self::Xml(err) => write!(formatter, "error parsing XML: {err}"),
        }
    }
}

impl StdError for ParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Xml(err) => Some(err),
            Self::InvalidExitStatus(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct UserInputState {
    exit_status: Option<ExitStatus>,
    text: TextReadingState,
    prompt: Option<Cow<'static, str>>,
    prompt_open_tags: Option<usize>,
}

impl UserInputState {
    fn new(exit_status: Option<ExitStatus>) -> Self {
        Self {
            exit_status,
            text: TextReadingState::default(),
            prompt: None,
            prompt_open_tags: None,
        }
    }
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

    fn process(&mut self, event: Event<'_>) -> Result<Option<Interaction<Parsed>>, ParseError> {
        let mut is_prompt_end = false;
        if let Event::Start(tag) = &event {
            if self.can_start_prompt() && parse_classes(tag.attributes())?.as_ref() == b"prompt" {
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
                let input = UserInput {
                    text: String::new(),
                    prompt: Some(UserInput::intern_prompt(parsed.plaintext)),
                    hidden: false,
                };
                return Ok(Some(Interaction {
                    input,
                    output: Parsed::default(),
                    exit_status: self.exit_status,
                }));
            }
            let text = mem::take(&mut self.text.plaintext_buffer);
            self.prompt = Some(UserInput::intern_prompt(text));
        }

        Ok(maybe_parsed.map(|parsed| {
            let input = UserInput {
                text: parsed.into_input_text(),
                prompt: self.prompt.take(),
                hidden: false,
            };
            Interaction {
                input,
                output: Parsed::default(),
                exit_status: self.exit_status,
            }
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
    /// Encountered `<div class="container">`; searching for `<div class="input">`.
    EncounteredContainer,
    /// Reading user input (`<div class="input">` contents).
    ReadingUserInput(UserInputState),
    /// Finished reading user input; searching for `<div class="output">`.
    EncounteredUserInput(Interaction<Parsed>),
    /// Reading terminal output (`<div class="output">` contents).
    ReadingTermOutput(Interaction<Parsed>, TextReadingState),
}

impl ParserState {
    const DUMMY_INTERACTION: Interaction<Parsed> = Interaction {
        input: UserInput {
            text: String::new(),
            prompt: None,
            hidden: false,
        },
        output: Parsed::DEFAULT,
        exit_status: None,
    };

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    fn set_state(&mut self, new_state: Self) {
        *self = new_state;
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", err))]
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
                        self.set_state(Self::EncounteredContainer);
                    }
                }
            }

            Self::EncounteredContainer => {
                if let Event::Start(tag) = event {
                    let classes = parse_classes(tag.attributes())?;
                    if Self::is_input_class(extract_base_class(&classes)) {
                        let exit_status = parse_exit_status(tag.attributes())?;
                        self.set_state(Self::ReadingUserInput(UserInputState::new(exit_status)));
                    }
                }
            }

            Self::ReadingUserInput(state) => {
                if let Some(interaction) = state.process(event)? {
                    self.set_state(Self::EncounteredUserInput(interaction));
                }
            }

            Self::EncounteredUserInput(interaction) => {
                if let Event::Start(tag) = event {
                    let classes = parse_classes(tag.attributes())?;
                    let base_class = extract_base_class(&classes);

                    if Self::is_output_class(base_class) {
                        let interaction = mem::replace(interaction, Self::DUMMY_INTERACTION);
                        self.set_state(Self::ReadingTermOutput(
                            interaction,
                            TextReadingState::default(),
                        ));
                    } else if Self::is_input_class(base_class) {
                        let interaction = mem::replace(interaction, Self::DUMMY_INTERACTION);
                        let exit_status = parse_exit_status(tag.attributes())?;
                        self.set_state(Self::ReadingUserInput(UserInputState::new(exit_status)));
                        return Ok(Some(interaction));
                    }
                }
            }

            Self::ReadingTermOutput(interaction, text_state) => {
                if let Some(term_output) = text_state.process(event)? {
                    let mut interaction = mem::replace(interaction, Self::DUMMY_INTERACTION);
                    interaction.output = term_output;
                    self.set_state(Self::EncounteredContainer);
                    return Ok(Some(interaction));
                }
            }
        }
        Ok(None)
    }

    fn is_input_class(class_name: &[u8]) -> bool {
        class_name == b"input" || class_name == b"user-input"
    }

    fn is_output_class(class_name: &[u8]) -> bool {
        class_name == b"output" || class_name == b"term-output"
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip_all, err)
    )]
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
