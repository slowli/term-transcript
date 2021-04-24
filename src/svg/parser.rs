use quick_xml::{
    events::{attributes::Attributes, Event},
    Reader as XmlReader,
};

use std::{
    borrow::Cow,
    error::Error as StdError,
    fmt,
    io::{self, BufRead},
    mem, str,
};

use crate::{Interaction, Parsed, Transcript, UserInput};

/// Errors that can occur during parsing SVG transcripts.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Unexpected root XML tag; must be `<svg>`.
    UnexpectedRoot,
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
            Self::UnexpectedRoot => formatter.write_str("Unexpected root XML tag; expected <svg>"),
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

#[derive(Debug, Default)]
struct TextReadingState {
    html_buffer: String,
    plaintext_buffer: String,
    open_tags: usize,
}

impl TextReadingState {
    // We only retain `<span>` tags in the HTML since they are the only ones containing color info.
    fn process(&mut self, event: Event<'_>) -> Result<Option<Parsed>, ParseError> {
        match event {
            Event::Text(text) => {
                let unescaped = text.unescaped()?;
                let unescaped_str = str::from_utf8(&unescaped).map_err(quick_xml::Error::Utf8)?;
                self.html_buffer.push_str(unescaped_str);
                self.plaintext_buffer.push_str(unescaped_str);
            }
            Event::Start(tag) => {
                self.open_tags += 1;
                if tag.name() == b"span" {
                    self.html_buffer.push('<');
                    let tag_str = str::from_utf8(&tag).map_err(quick_xml::Error::Utf8)?;
                    self.html_buffer.push_str(tag_str);
                    self.html_buffer.push('>');
                }
            }
            Event::End(tag) => {
                self.open_tags -= 1;

                if tag.name() == b"span" {
                    self.html_buffer.push_str("</span>");
                }

                if self.open_tags == 0 {
                    let html = mem::take(&mut self.html_buffer);
                    let plaintext = mem::take(&mut self.plaintext_buffer);
                    return Ok(Some(Parsed { plaintext, html }));
                }
            }
            _ => { /* Do nothing */ }
        }
        Ok(None)
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
                        return Err(ParseError::UnexpectedRoot);
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

#[cfg(test)]
mod tests {
    use super::*;

    use assert_matches::assert_matches;
    use quick_xml::events::{BytesEnd, BytesStart, BytesText};

    use std::io::{Cursor, Read};

    const SVG: &[u8] = br#"
        <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
          <foreignObject x="0" y="0" width="652" height="344">
            <div xmlns="http://www.w3.org/1999/xhtml" class="container">
              <div class="user-input"><pre><span class="prompt">$</span> ls -al --color=always</pre></div>
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
            UserInput { text, prompt }
                if text == "ls -al --color=always" && prompt.as_deref() == Some("$")
        );

        let plaintext = &interaction.output.plaintext;
        assert!(plaintext.starts_with("total 28\ndrwxr-xr-x"));
        assert!(plaintext.contains("4096 Apr 18 12:54 .\n"));
        assert!(!plaintext.contains(r#"<span class="fg-blue">.</span>"#));

        let html = &interaction.output.html;
        assert!(html.starts_with("total 28\ndrwxr-xr-x"));
        assert!(html.contains(r#"Apr 18 12:54 <span class="fg-blue">.</span>"#));
    }

    #[test]
    fn reading_file_with_extra_info() {
        let mut data = SVG.to_owned();
        data.extend_from_slice(b"<other>data</other>");
        let mut data = Cursor::new(data.as_slice());

        let transcript = Transcript::from_svg(&mut data).unwrap();
        assert_eq!(transcript.interactions.len(), 1);

        // Check that the parser stops after `</svg>`.
        let mut end = String::new();
        data.read_to_string(&mut end).unwrap();
        assert_eq!(end.trim_start(), "<other>data</other>");
    }

    #[test]
    fn reading_file_without_svg_tag() {
        let data: &[u8] = b"<div>Text</div>";
        let err = Transcript::from_svg(data).unwrap_err();

        assert_matches!(err, ParseError::UnexpectedRoot);
    }

    #[test]
    fn reading_file_without_container() {
        let bogus_data: &[u8] = br#"
            <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
              <style>.container { color: #eee; }</style>
            </svg>
        "#;
        let err = Transcript::from_svg(bogus_data).unwrap_err();

        assert_matches!(err, ParseError::UnexpectedEof);
    }

    #[test]
    fn reading_file_with_invalid_container() {
        const INVALID_ATTRS: &[&str] = &[
            "",
            // no class
            r#"xmlns="http://www.w3.org/1999/xhtml""#,
            // no namespace
            r#"class="container""#,
            // invalid namespace
            r#"xmlns="http://www.w3.org/2000/svg" class="container""#,
            // invalid class
            r#"xmlns="http://www.w3.org/1999/xhtml" class="cont""#,
        ];

        for &attrs in INVALID_ATTRS {
            let bogus_data = format!(
                r#"
                <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
                  <foreignObject x="0" y="0" width="652" height="344">
                    <div {}>Test</div>
                  </foreignObject>
                </svg>
                "#,
                attrs
            );
            let err = Transcript::from_svg(bogus_data.as_bytes()).unwrap_err();

            assert_matches!(err, ParseError::InvalidContainer);
        }
    }

    #[test]
    fn reading_file_without_term_output() {
        let bogus_data: &[u8] = br#"
            <svg viewBox="0 0 652 344" xmlns="http://www.w3.org/2000/svg" version="1.1">
              <foreignObject x="0" y="0" width="652" height="344">
                <div xmlns="http://www.w3.org/1999/xhtml" class="container">
                  <div class="user-input"><pre>$ ls -al --color=always</pre></div>
                </div>
              </foreignObject>
            </svg>
        "#;
        let err = Transcript::from_svg(bogus_data).unwrap_err();

        assert_matches!(err, ParseError::UnexpectedEof);
    }

    #[test]
    fn reading_user_input_with_manual_events() {
        let mut state = UserInputState::default();
        {
            let event = Event::Start(BytesStart::borrowed_name(b"pre"));
            assert!(state.process(event).unwrap().is_none());
            assert_eq!(state.prompt_open_tags, None);
            assert!(state.text.plaintext_buffer.is_empty());
        }
        {
            let event = Event::Start(BytesStart::borrowed(br#"span class="prompt""#, 4));
            assert!(state.process(event).unwrap().is_none());
            assert_eq!(state.prompt_open_tags, Some(1));
            assert!(state.text.plaintext_buffer.is_empty());
        }
        {
            let event = Event::Text(BytesText::from_escaped(b"$" as &[u8]));
            assert!(state.process(event).unwrap().is_none());
            assert_eq!(state.text.plaintext_buffer, "$");
        }
        {
            let event = Event::End(BytesEnd::borrowed(b"span"));
            assert!(state.process(event).unwrap().is_none());
            assert_eq!(state.prompt.as_deref(), Some("$"));
            assert!(state.text.plaintext_buffer.is_empty());
        }
        {
            let event = Event::Text(BytesText::from_escaped(b" rainbow" as &[u8]));
            assert!(state.process(event).unwrap().is_none());
        }

        let event = Event::End(BytesEnd::borrowed(b"pre"));
        let user_input = state.process(event).unwrap().unwrap();
        assert_eq!(user_input.prompt(), Some("$"));
        assert_eq!(user_input.text, "rainbow");
    }

    fn read_user_input(input: &[u8]) -> UserInput {
        let mut reader = XmlReader::from_reader(Cursor::new(input));
        let mut buffer = vec![];
        let mut state = UserInputState::default();

        loop {
            let event = reader.read_event(&mut buffer).unwrap();
            if let Event::Eof = &event {
                panic!("Reached EOF without creating `UserInput`");
            }

            if let Some(user_input) = state.process(event).unwrap() {
                break user_input;
            }
        }
    }

    #[test]
    fn reading_user_input_base_case() {
        let user_input =
            read_user_input(br#"<pre><span class="prompt">&gt;</span> echo foo</pre>"#);

        assert_eq!(user_input.prompt.as_deref(), Some(">"));
        assert_eq!(user_input.text, "echo foo");
    }

    #[test]
    fn reading_user_input_without_prompt() {
        let user_input = read_user_input(br#"<pre>echo <span class="bold">foo</span></pre>"#);

        assert_eq!(user_input.prompt.as_deref(), None);
        assert_eq!(user_input.text, "echo foo");
    }

    #[test]
    fn reading_user_input_with_prompt_only() {
        let user_input = read_user_input(br#"<pre><span class="prompt">$</span></pre>"#);

        assert_eq!(user_input.prompt.as_deref(), Some("$"));
        assert_eq!(user_input.text, "");
    }

    #[test]
    fn reading_user_input_with_bogus_prompt_location() {
        let user_input =
            read_user_input(br#"<pre>echo foo <span class="prompt">&gt;</span> output.log</pre>"#);

        assert_eq!(user_input.prompt.as_deref(), None);
        assert_eq!(user_input.text, "echo foo > output.log");
    }

    #[test]
    fn reading_user_input_with_multiple_prompts() {
        let user_input = read_user_input(
            b"<pre><span class=\"prompt\">&gt;&gt;&gt;</span>  \
                    echo foo <span class=\"prompt\">&gt;</span> output.log</pre>",
        );

        assert_eq!(user_input.prompt.as_deref(), Some(">>>"));
        assert_eq!(user_input.text, "echo foo > output.log");
    }
}
