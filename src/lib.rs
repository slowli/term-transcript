//! Snapshot testing for CLI / REPL applications, in a fun way.
//!
//! # What it does
//!
//! This crate allows to:
//!
//! - Create [`Transcript`]s of interacting with a terminal, capturing both the output text
//!   and [ANSI-compatible color info][SGR].
//! - Save these transcripts in the [SVG] format, so that they can be easily embedded as images
//!   into HTML / Markdown documents
//! - Parse transcripts from SVG
//! - Test that a parsed transcript actually corresponds to the terminal output (either as text
//!   or text + colors).
//!
//! The primary use case is easy to create and maintain end-to-end tests for CLI / REPL apps.
//! Such tests can be embedded into a readme file.
//!
//! # Design decisions
//!
//! - **Static capturing.** Capturing dynamic interaction with the terminal essentially
//!   requires writing / hacking together a new terminal, which looks like an overkill
//!   for the motivating use case (snapshot testing).
//!
//! - **(Primarily) static SVGs.** Animated SVGs create visual noise and make simple things
//!   (e.g., copying text from an SVG) harder than they should be.
//!
//! - **Self-contained tests.** Unlike generic snapshot files, [`Transcript`]s contain
//!   both user inputs and outputs. This allows using them as images with little additional
//!   explanation.
//!
//! # Limitations
//!
//! - Terminal coloring only works with ANSI escape codes. (Since ANSI escape codes
//!   are supported even on Windows nowadays, this shouldn't be a significant problem.)
//! - ANSI escape sequences other than [SGR] ones are either dropped (in case of [CSI] sequences),
//!   or lead to [`TermError::NonCsiSequence`].
//! - Since the terminal is not not emulated, programs dependent on [`isatty`] checks can produce
//!   different output than if launched in the actual shell. One can argue that dependence
//!   on `isatty` is generally an anti-pattern.
//! - As a consequence of the last point, CLI tools frequently switch off output coloring if not
//!   writing to a terminal. For some tools, this can be amended by adding an arg to the command,
//!   such as `--color=always`.
//!
//! # Alternatives / similar tools
//!
//! - [`insta`](https://crates.io/crates/insta) is a generic snapshot testing library, which
//!   is amazing in general, but *kind of* too low-level for E2E CLI testing.
//! - [`trybuild`](https://crates.io/crates/trybuild) snapshot-tests output
//!   of a particular program (the Rust compiler).
//! - Tools like [`termtosvg`](https://github.com/nbedos/termtosvg) and
//!   [Asciinema](https://asciinema.org/) allow to record terminal sessions and save them to SVG.
//!   The output of these tools is inherently *dynamic* (which, e.g., results in animated SVGs).
//!   This crate [intentionally chooses](#design-decisions) a simpler static format, which
//!   makes snapshot testing easier.
//!
//! [SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
//! [SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
//! [CSI]: https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences
//! [`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
//!
//! # Examples
//!
//! Creating a terminal [`Transcript`] and rendering it to SVG.
//!
//! ```
//! use term_svg::{
//!     svg::{Template, TemplateOptions}, ShellOptions, Transcript, UserInput,
//! };
//! # use std::str;
//!
//! # fn main() -> anyhow::Result<()> {
//! let transcript = Transcript::from_inputs(
//!     &mut ShellOptions::default(),
//!     vec![UserInput::command(r#"echo "Hello world!""#)],
//! )?;
//! let mut writer = vec![];
//! // ^ Any `std::io::Write` implementation will do, such as a `File`.
//! Template::new(TemplateOptions::default()).render(&transcript, &mut writer)?;
//! println!("{}", str::from_utf8(&writer)?);
//! # Ok(())
//! # }
//! ```
//!
//! Loading a `Transcript` and testing it. See the [`test` module](crate::test) for more examples.
//!
//! ```
//! use term_svg::{test::TestConfig, ShellOptions, Transcript, UserInput};
//! # use term_svg::svg::{Template, TemplateOptions};
//! use std::io;
//!
//! fn read_svg_file() -> anyhow::Result<impl io::BufRead> {
//!     // snipped...
//! #   let transcript = Transcript::from_inputs(
//! #        &mut ShellOptions::default(),
//! #        vec![UserInput::command(r#"echo "Hello world!""#)],
//! #   )?;
//! #   let mut writer = vec![];
//! #   Template::new(TemplateOptions::default()).render(&transcript, &mut writer)?;
//! #   Ok(io::Cursor::new(writer))
//! }
//!
//! # fn main() -> anyhow::Result<()> {
//! let reader = read_svg_file()?;
//! let transcript = Transcript::from_svg(reader)?;
//! TestConfig::new(ShellOptions::default()).test_transcript(&transcript);
//! # Ok(())
//! # }
//! ```

// Documentation settings.
#![cfg_attr(docsrs, feature(doc_cfg))]
// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

use serde::Serialize;

use std::{borrow::Cow, error::Error as StdError, fmt, io, num::ParseIntError};

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
#[derive(Debug)]
#[non_exhaustive]
pub enum TermError {
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

impl fmt::Display for TermError {
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

impl StdError for TermError {
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
        let mut input_lines = bytecount::count(self.input.text.as_bytes(), b'\n');
        if !self.input.text.ends_with('\n') {
            input_lines += 1;
        }

        let mut output_lines = bytecount::count(self.output.as_ref(), b'\n');
        if !self.output.as_ref().ends_with(b"\n") {
            output_lines += 1;
        }

        input_lines + output_lines
    }
}

/// User input during interaction with a terminal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserInput {
    text: String,
    prompt: Option<Cow<'static, str>>,
}

impl UserInput {
    pub(crate) fn intern_prompt(prompt: String) -> Cow<'static, str> {
        match prompt.as_str() {
            "$" => Cow::Borrowed("$"),
            ">>>" => Cow::Borrowed(">>>"),
            "..." => Cow::Borrowed("..."),
            _ => Cow::Owned(prompt),
        }
    }

    /// Creates a command.
    pub fn command(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            prompt: Some(Cow::Borrowed("$")),
        }
    }

    /// Gets the kind of this input.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }
}

impl AsRef<str> for UserInput {
    fn as_ref(&self) -> &str {
        &self.text
    }
}
