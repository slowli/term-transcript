//! Snapshot testing for CLI / REPL applications, in a fun way.

// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

use serde::Serialize;

use std::{error::Error as StdError, fmt, io, num::ParseIntError, str::FromStr};

mod html;
mod shell;
pub mod svg;
mod term;
pub mod test;
mod utils;

pub use self::{
    shell::{ShellOptions, StdShell},
    term::{Captured, Parsed, TermOutput},
};

/// Errors that can occur when processing terminal output.
// FIXME: rename to `TermError`
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
    pub fn add_interaction(&mut self, input: UserInput, output: Vec<u8>) -> &mut Self {
        self.interactions.push(Interaction {
            input,
            output: Captured::new(output),
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
        let additional_lines = if self.output.as_ref().ends_with(b"\n") {
            1
        } else {
            2
        };
        bytecount::count(self.output.as_ref(), b'\n') + additional_lines
    }
}

/// User input during interaction with a terminal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserInput {
    text: String,
    kind: UserInputKind,
}

/// Kind of user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserInputKind {
    /// Standalone shell command.
    Command,
    /// Input into an interactive session
    Repl,
}

impl UserInputKind {
    fn from_prompt(prompt: &str) -> Option<Self> {
        match prompt {
            "$" => Some(Self::Command),
            ">>>" => Some(Self::Repl),
            _ => None,
        }
    }
}

impl UserInput {
    /// Creates a command.
    pub fn command(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: UserInputKind::Command,
        }
    }

    /// Gets the kind of this input.
    pub fn kind(&self) -> UserInputKind {
        self.kind
    }
}

impl AsRef<str> for UserInput {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl FromStr for UserInput {
    type Err = UserInputParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.splitn(2, |c: char| c.is_ascii_whitespace()).collect();
        match parts.as_slice() {
            [prompt, text] => Ok(Self {
                kind: UserInputKind::from_prompt(prompt)
                    .ok_or(UserInputParseError::UnrecognizedPrefix)?,
                text: (*text).to_owned(),
            }),
            _ => Err(UserInputParseError::NoPrefix),
        }
    }
}

/// Errors that can occur during parsing [`UserInput`]s.
#[derive(Debug)]
#[non_exhaustive]
pub enum UserInputParseError {
    /// No input prefix (e.g., `$ ` in `$ ls -al`).
    NoPrefix,
    /// Unrecognized input prefix.
    UnrecognizedPrefix,
}

impl fmt::Display for UserInputParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPrefix => formatter.write_str("No input prefix"),
            Self::UnrecognizedPrefix => formatter.write_str("Unrecognized input prefix"),
        }
    }
}

impl StdError for UserInputParseError {}
