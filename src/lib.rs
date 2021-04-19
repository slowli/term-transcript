//! Snapshot testing for CLI / REPL applications, in a fun way.

// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

use handlebars::Output;
use serde::Serialize;

use std::{error::Error as StdError, fmt, io, num::ParseIntError};

mod html;
mod template;
mod term;

pub use self::template::{SvgTemplate, SvgTemplateOptions};

use self::{html::HtmlWriter, term::TermOutputParser};

/// Marker trait for supported types of terminal output.
pub trait TermOutput: Clone + Send + Sync + 'static {}

/// Output captured from the terminal.
#[derive(Debug, Clone)]
pub struct Captured(pub Vec<u8>);

impl TermOutput for Captured {}

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
#[derive(Debug, Clone)]
pub struct Transcript<Out: TermOutput = Captured> {
    interactions: Vec<Interaction<Out>>,
}

impl<Out: TermOutput> Default for Transcript<Out> {
    fn default() -> Self {
        Self {
            interactions: vec![],
        }
    }
}

impl<Out: TermOutput> Transcript<Out> {
    /// Creates an empty transcript.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns interactions in this transcript.
    pub fn interactions(&self) -> &[Interaction<Out>] {
        &self.interactions
    }
}

impl Transcript {
    /// Adds a new interaction into the transcript.
    pub fn add_interaction(&mut self, input: UserInput, output: impl Into<Vec<u8>>) -> &mut Self {
        self.interactions.push(Interaction {
            input,
            output: Captured(output.into()),
        });
        self
    }
}

/// One-time interaction with the terminal.
#[derive(Debug, Clone)]
pub struct Interaction<Out: TermOutput = Captured> {
    input: UserInput,
    output: Out,
}

impl<Out: TermOutput> Interaction<Out> {
    /// Input provided by the user.
    pub fn input(&self) -> &UserInput {
        &self.input
    }

    /// Output to the terminal.
    pub fn output(&self) -> &Out {
        &self.output
    }
}

impl Interaction<Captured> {
    /// Counts the total number of lines in input and output.
    pub fn count_lines(&self) -> usize {
        let additional_lines = if self.output.0.ends_with(b"\n") { 1 } else { 2 };
        bytecount::count(&self.output.0, b'\n') + additional_lines
    }

    /// Writes terminal [`output`](Self::output()) in the HTML format to the provided `writer`.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn write_html_output(&self, writer: &mut dyn Output) -> Result<(), Error> {
        let mut html_writer = HtmlWriter::new(writer);
        TermOutputParser::new(&mut html_writer).parse(&self.output.0)
    }
}

/// User input during interaction with a terminal.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserInput {
    /// Executing the specified command.
    Command(String),
}

impl UserInput {
    /// Creates a command.
    pub fn command(val: impl Into<String>) -> Self {
        Self::Command(val.into())
    }
}
