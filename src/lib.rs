//! Snapshot testing for CLI / REPL applications, in a fun way.

// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

use handlebars::Output;
use serde::Serialize;

use std::{error::Error as StdError, fmt, io, num::ParseIntError};

mod parser;
mod template;
mod writer;

pub use self::template::{SvgTemplate, SvgTemplateOptions};

use self::{parser::TermOutputParser, writer::HtmlWriter};

/// Errors that can occur when processing terminal output.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Unfinished escape sequence.
    UnfinishedSequence,
    /// Non-CSI escape sequence. The enclosed byte is the first byte of the sequence (excluding
    /// `0x1b`).
    NonCsiSequence(u8),
    /// Invalid final byte for an SGR escape sequence.
    InvalidSgrFinalByte(u8),
    /// Unfinished color spec.
    UnfinishedColor,
    /// Invalid type of a color spec.
    InvalidColorType(String),
    /// Invalid ANSI color index.
    InvalidColorIndex(ParseIntError),
    /// IO error.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnfinishedSequence => formatter.write_str("Unfinished ANSI escape sequence"),
            Self::NonCsiSequence(byte) => {
                write!(
                    formatter,
                    "Non-CSI escape sequence (first byte is {})",
                    byte
                )
            }
            Self::InvalidSgrFinalByte(byte) => {
                write!(
                    formatter,
                    "Invalid final byte for an SGR escape sequence: {}",
                    byte
                )
            }
            Self::UnfinishedColor => formatter.write_str("Unfinished color spec"),
            Self::InvalidColorType(ty) => {
                write!(formatter, "Invalid type of a color spec: {}", ty)
            }
            Self::InvalidColorIndex(err) => {
                write!(formatter, "Failed parsing color index: {}", err)
            }
            Self::Io(err) => write!(formatter, "I/O error: {}", err),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::InvalidColorIndex(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

/// Transcript of a user interacting with the terminal.
#[derive(Debug, Clone, Default)]
pub struct Transcript<'a> {
    interactions: Vec<Interaction<'a>>,
}

impl<'a> Transcript<'a> {
    /// Creates an empty transcript.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns interactions in this transcript.
    pub fn interactions(&self) -> &[Interaction<'a>] {
        &self.interactions
    }

    /// Adds a new interaction into the transcript.
    pub fn add_interaction(&mut self, input: UserInput<'a>, output: &'a [u8]) -> &mut Self {
        self.interactions.push(Interaction { input, output });
        self
    }
}

/// One-time interaction with the terminal.
#[derive(Debug, Clone, Copy)]
pub struct Interaction<'a> {
    input: UserInput<'a>,
    output: &'a [u8],
}

impl<'a> Interaction<'a> {
    /// Input provided by the user.
    pub fn input(self) -> UserInput<'a> {
        self.input
    }

    /// Output to the terminal.
    pub fn output(self) -> &'a [u8] {
        self.output
    }

    /// Counts the total number of lines in input and output.
    pub fn count_lines(self) -> usize {
        let additional_lines = if self.output.ends_with(b"\n") { 1 } else { 2 };
        bytecount::count(self.output, b'\n') + additional_lines
    }

    /// Writes terminal [`output`](Self::output()) in the HTML format to the provided `writer`.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn write_html_output(self, writer: &mut dyn Output) -> Result<(), Error> {
        let mut html_writer = HtmlWriter::new(writer);
        TermOutputParser::new(&mut html_writer).parse(self.output)
    }
}

/// User input during interaction with a terminal.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserInput<'a> {
    /// Executing the specified command.
    Command(&'a str),
}
