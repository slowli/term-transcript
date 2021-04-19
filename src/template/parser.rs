use quick_xml::{
    events::{attributes::Attributes, Event},
    Reader as XmlReader,
};

use std::{borrow::Cow, io::BufRead, mem};

use crate::{Interaction, TermOutput, Transcript, UserInput, UserInputParseError};

#[derive(Debug, Clone)]
pub enum Parsed {
    Plaintext(String),
    Html(Vec<u8>),
}

impl TermOutput for Parsed {}

#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    InvalidRoot,
    InvalidContainer,
    InvalidUserInput(String, UserInputParseError),
    UnexpectedEof,
    Xml(quick_xml::Error),
}

impl From<quick_xml::Error> for ParseError {
    fn from(err: quick_xml::Error) -> Self {
        Self::Xml(err)
    }
}

#[derive(Debug)]
enum ParserState {
    /// Initial state.
    Initialized,
    /// Encountered `<svg>` tag; searching for `<div class="container">`.
    EncounteredSvgTag,
    /// Encountered `<div class="container">`; searching for `<div class="user-input">`.
    EncounteredContainer,
    /// Reading user input (`<div class="user-input">` contents).
    ReadingUserInput(TextReadingState),
    /// Finished reading user input; searching for `<div class="term-output">`.
    EncounteredUserInput(UserInput),
    /// Reading terminal output (`<div class="term-output">` contents).
    ReadingTermOutput(UserInput, TextReadingState),
}

#[derive(Debug, Default)]
struct TextReadingState {
    buffer: Vec<u8>,
    open_tags: usize,
}

impl TextReadingState {
    fn process(&mut self, event: Event<'_>) -> Result<Option<String>, ParseError> {
        match event {
            Event::Text(text) => {
                self.buffer.extend_from_slice(text.unescaped()?.as_ref());
            }
            Event::Start(_) => {
                self.open_tags += 1;
            }
            Event::End(_) => {
                self.open_tags -= 1;
                if self.open_tags == 0 {
                    let buffer = mem::take(&mut self.buffer);
                    let buffer = String::from_utf8(buffer)
                        .map_err(|err| quick_xml::Error::Utf8(err.utf8_error()))?;
                    return Ok(Some(buffer));
                }
            }
            _ => { /* Do nothing */ }
        }
        Ok(None)
    }
}

impl ParserState {
    const DUMMY_INPUT: UserInput = UserInput::Command(String::new());

    fn process(&mut self, event: Event<'_>) -> Result<Option<Interaction<Parsed>>, ParseError> {
        match self {
            Self::Initialized => {
                if let Event::Start(tag) = event {
                    if tag.name() == b"svg" {
                        *self = Self::EncounteredSvgTag;
                    } else {
                        return Err(ParseError::InvalidRoot);
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
                        *self = Self::ReadingUserInput(TextReadingState::default());
                    }
                }
            }

            Self::ReadingUserInput(text_state) => {
                if let Some(user_input) = text_state.process(event)? {
                    let user_input = user_input
                        .parse()
                        .map_err(|err| ParseError::InvalidUserInput(user_input, err))?;
                    *self = Self::EncounteredUserInput(user_input);
                }
            }

            Self::EncounteredUserInput(user_input) => {
                if let Event::Start(tag) = event {
                    if Self::get_class(tag.attributes())?.as_ref() == b"term-output" {
                        let user_input = mem::replace(user_input, Self::DUMMY_INPUT);
                        *self = Self::ReadingTermOutput(user_input, TextReadingState::default());
                    }
                }
            }

            Self::ReadingTermOutput(user_input, text_state) => {
                if let Some(term_output) = text_state.process(event)? {
                    let user_input = mem::replace(user_input, Self::DUMMY_INPUT);
                    *self = Self::EncounteredContainer;

                    return Ok(Some(Interaction {
                        input: user_input,
                        output: Parsed::Plaintext(term_output),
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
    pub fn from_svg<R: BufRead>(reader: R) -> Result<Self, ParseError> {
        let mut reader = XmlReader::from_reader(reader);
        let mut buffer = vec![];
        let mut state = ParserState::Initialized;
        let mut transcript = Self::new();

        loop {
            let event = reader.read_event(&mut buffer)?;

            if let Event::Eof = event {
                if let ParserState::EncounteredContainer = state {
                    break;
                }
                return Err(ParseError::UnexpectedEof);
            }

            if let Some(interaction) = state.process(event)? {
                transcript.interactions.push(interaction);
            }
        }
        Ok(transcript)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_matches::assert_matches;

    const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="user-input"><pre><span class="bold">$</span> ls -al --color=always</pre></div>
              <div class="term-output"><pre>total 28
drwxr-xr-x 1 alex alex 4096 Apr 18 12:54 <span class="fg-blue">.</span>
drwxrwxrwx 1 alex alex 4096 Apr 18 12:38 <span class="fg-blue bg-green">..</span>
-rw-r--r-- 1 alex alex 8199 Apr 18 12:48 Cargo.lock</pre>
              </div>
            </div>
          </foreignObject>
        </svg>
    "#;

    #[test]
    fn reading_file() {
        let transcript = Transcript::from_svg(SVG).unwrap();
        assert_eq!(transcript.interactions.len(), 1);

        let interaction = &transcript.interactions[0];
        assert_matches!(
            &interaction.input,
            UserInput::Command(cmd) if cmd == "$ ls -al --color=always"
        );
        assert_matches!(
            &interaction.output,
            Parsed::Plaintext(out) if out.starts_with("total 28\ndrwxr-xr-x") &&
                out.contains("4096 Apr 18 12:54 .\n")
        );
    }

    // TODO: test errors
}
