//! Snapshot testing for CLI / REPL applications, in a fun way.
//!
//! # What it does
//!
//! This crate allows to:
//!
//! - Create [`Transcript`]s of interacting with a terminal, capturing both the output text
//!   and [ANSI-compatible color info][SGR].
//! - Save these transcripts in the [SVG] format, so that they can be easily embedded as images
//!   into HTML / Markdown documents. (Output format customization
//!   [is also supported](svg::Template#customization) via [Handlebars] templates.)
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
//! - ANSI escape sequences other than [SGR] ones are either dropped (in case of [CSI]
//!   and OSC sequences), or lead to [`TermError::UnrecognizedSequence`].
//! - By default, the crate exposes APIs to perform capture via OS pipes.
//!   Since the terminal is not emulated in this case, programs dependent on [`isatty`] checks
//!   or getting term size can produce different output than if launched in an actual shell
//!   (no coloring, no line wrapping etc.).
//! - It is possible to capture output from a pseudo-terminal (PTY) using the `portable-pty`
//!   crate feature. However, since most escape sequences are dropped, this is still not a good
//!   option to capture complex outputs (e.g., ones moving cursor).
//!
//! # Alternatives / similar tools
//!
//! - [`insta`](https://crates.io/crates/insta) is a generic snapshot testing library, which
//!   is amazing in general, but *kind of* too low-level for E2E CLI testing.
//! - [`rexpect`](https://crates.io/crates/rexpect) allows testing CLI / REPL applications
//!   by scripting interactions with them in tests. It works in Unix only.
//! - [`trybuild`](https://crates.io/crates/trybuild) snapshot-tests output
//!   of a particular program (the Rust compiler).
//! - [`trycmd`](https://crates.io/crates/trycmd) snapshot-tests CLI apps using
//!   a text-based format.
//! - Tools like [`termtosvg`](https://github.com/nbedos/termtosvg) and
//!   [Asciinema](https://asciinema.org/) allow recording terminal sessions and save them to SVG.
//!   The output of these tools is inherently *dynamic* (which, e.g., results in animated SVGs).
//!   This crate [intentionally chooses](#design-decisions) a simpler static format, which
//!   makes snapshot testing easier.
//!
//! # Crate features
//!
//! ## `portable-pty`
//!
//! *(Off by default)*
//!
//! Allows using pseudo-terminal (PTY) to capture terminal output rather than pipes.
//! Uses [the eponymous crate][`portable-pty`] under the hood.
//!
//! ## `svg`
//!
//! *(On by default)*
//!
//! Exposes [the eponymous module](svg) that allows rendering [`Transcript`]s
//! into the SVG format.
//!
//! ## `test`
//!
//! *(On by default)*
//!
//! Exposes [the eponymous module](crate::test) that allows parsing [`Transcript`]s
//! from SVG files and testing them.
//!
//! ## `pretty_assertions`
//!
//! *(On by default)*
//!
//! Uses [the eponymous crate][`pretty_assertions`] when testing SVG files.
//! Only really makes sense together with the `test` feature.
//!
//! ## `tracing`
//!
//! *(Off by default)*
//!
//! Uses [the eponymous facade][`tracing`] to trace main operations, which could be useful
//! for debugging. Tracing is mostly performed on the `DEBUG` level.
//!
//! [SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
//! [SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
//! [CSI]: https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences
//! [`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
//! [Handlebars]: https://handlebarsjs.com/
//! [`pretty_assertions`]: https://docs.rs/pretty_assertions/
//! [`portable-pty`]: https://docs.rs/portable-pty/
//! [`tracing`]: https://docs.rs/tracing/
//!
//! # Examples
//!
//! Creating a terminal [`Transcript`] and rendering it to SVG.
//!
//! ```
//! use term_transcript::{
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
//! Snapshot testing. See the [`test` module](crate::test) for more examples.
//!
//! ```
//! use term_transcript::{test::TestConfig, ShellOptions};
//!
//! #[test]
//! fn echo_works() {
//!     TestConfig::new(ShellOptions::default()).test(
//!         "tests/__snapshots__/echo.svg",
//!         &[r#"echo "Hello world!""#],
//!     );
//! }
//! ```

// Documentation settings.
#![doc(html_root_url = "https://docs.rs/term-transcript/0.3.0-beta.1")]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

use std::{borrow::Cow, error::Error as StdError, fmt, io, num::ParseIntError};

#[cfg(feature = "portable-pty")]
mod pty;
mod shell;
#[cfg(feature = "svg")]
#[cfg_attr(docsrs, doc(cfg(feature = "svg")))]
pub mod svg;
mod term;
#[cfg(feature = "test")]
#[cfg_attr(docsrs, doc(cfg(feature = "test")))]
pub mod test;
pub mod traits;
mod utils;
mod write;

#[cfg(feature = "portable-pty")]
pub use self::pty::{PtyCommand, PtyShell};
pub use self::{
    shell::{ShellOptions, StdShell},
    term::{Captured, TermOutput},
};

/// Errors that can occur when processing terminal output.
#[derive(Debug)]
#[non_exhaustive]
pub enum TermError {
    /// Unfinished escape sequence.
    UnfinishedSequence,
    /// Unrecognized escape sequence (not a CSI or OSC one). The enclosed byte
    /// is the first byte of the sequence (excluding `0x1b`).
    UnrecognizedSequence(u8),
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
            Self::UnrecognizedSequence(byte) => {
                write!(
                    formatter,
                    "Unrecognized escape sequence (first byte is {byte})"
                )
            }
            Self::InvalidSgrFinalByte(byte) => {
                write!(
                    formatter,
                    "Invalid final byte for an SGR escape sequence: {byte}"
                )
            }
            Self::UnfinishedColor => formatter.write_str("Unfinished color spec"),
            Self::InvalidColorType(ty) => {
                write!(formatter, "Invalid type of a color spec: {ty}")
            }
            Self::InvalidColorIndex(err) => {
                write!(formatter, "Failed parsing color index: {err}")
            }
            Self::Io(err) => write!(formatter, "I/O error: {err}"),
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
    /// Manually adds a new interaction to the end of this transcript.
    ///
    /// This method allows capturing interactions that are difficult or impossible to capture
    /// using more high-level methods: [`Self::from_inputs()`] or [`Self::capture_output()`].
    /// The resulting transcript will [render](svg) just fine, but there could be issues
    /// with [testing](crate::test) it.
    pub fn add_existing_interaction(&mut self, interaction: Interaction) -> &mut Self {
        self.interactions.push(interaction);
        self
    }

    /// Manually adds a new interaction to the end of this transcript.
    ///
    /// This is a shortcut for calling [`Self::add_existing_interaction(_)`].
    pub fn add_interaction(
        &mut self,
        input: impl Into<UserInput>,
        output: impl Into<String>,
    ) -> &mut Self {
        self.add_existing_interaction(Interaction::new(input, output))
    }
}

/// Portable, platform-independent version of [`ExitStatus`] from the standard library.
///
/// # Capturing `ExitStatus`
///
/// Some shells have means to check whether the input command was executed successfully.
/// For example, in `sh`-like shells, one can compare the value of `$?` to 0, and
/// in PowerShell to `True`. The exit status can be captured when creating a [`Transcript`]
/// by setting a *checker* in [`ShellOptions::with_status_check()`]:
///
/// # Examples
///
/// ```
/// # use term_transcript::{ExitStatus, ShellOptions, Transcript, UserInput};
/// # fn test_wrapper() -> anyhow::Result<()> {
/// let options = ShellOptions::default();
/// let mut options = options.with_status_check("echo $?", |captured| {
///     // Parse captured string to plain text. This transform
///     // is especially important in transcripts captured from PTY
///     // since they can contain a *wild* amount of escape sequences.
///     let captured = captured.to_plaintext().ok()?;
///     let code: i32 = captured.trim().parse().ok()?;
///     Some(ExitStatus(code))
/// });
///
/// let transcript = Transcript::from_inputs(&mut options, [
///     UserInput::command("echo \"Hello world\""),
///     UserInput::command("some-non-existing-command"),
/// ])?;
/// let status = transcript.interactions()[0].exit_status();
/// assert!(status.unwrap().is_success());
/// // The assertion above is equivalent to:
/// assert_eq!(status, Some(ExitStatus(0)));
///
/// let status = transcript.interactions()[1].exit_status();
/// assert!(!status.unwrap().is_success());
/// # Ok(())
/// # }
/// # // We can compile test in any case, but it successfully executes only on *nix.
/// # #[cfg(unix)] fn main() { test_wrapper().unwrap() }
/// # #[cfg(not(unix))] fn main() { }
/// ```
#[allow(clippy::doc_markdown)] // false positive on "PowerShell"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExitStatus(pub i32);

impl ExitStatus {
    /// Checks if this is the successful status.
    pub fn is_success(self) -> bool {
        self.0 == 0
    }
}

/// One-time interaction with the terminal.
#[derive(Debug, Clone)]
pub struct Interaction<Out: TermOutput = Captured> {
    input: UserInput,
    output: Out,
    exit_status: Option<ExitStatus>,
}

impl Interaction {
    /// Creates a new interaction.
    pub fn new(input: impl Into<UserInput>, output: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            output: Captured::new(output.into()),
            exit_status: None,
        }
    }

    /// Assigns an exit status to this interaction.
    #[must_use]
    pub fn with_exit_status(mut self, exit_status: ExitStatus) -> Self {
        self.exit_status = Some(exit_status);
        self
    }
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

    /// Returns exit status of the interaction, if available.
    pub fn exit_status(&self) -> Option<ExitStatus> {
        self.exit_status
    }
}

/// User input during interaction with a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "svg", derive(serde::Serialize))]
pub struct UserInput {
    text: String,
    prompt: Option<Cow<'static, str>>,
    hidden: bool,
}

impl UserInput {
    #[cfg(feature = "test")]
    pub(crate) fn intern_prompt(prompt: String) -> Cow<'static, str> {
        match prompt.as_str() {
            "$" => Cow::Borrowed("$"),
            ">>>" => Cow::Borrowed(">>>"),
            "..." => Cow::Borrowed("..."),
            _ => Cow::Owned(prompt),
        }
    }

    /// Creates a command input.
    pub fn command(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            prompt: Some(Cow::Borrowed("$")),
            hidden: false,
        }
    }

    /// Creates a standalone / starting REPL command input with the `>>>` prompt.
    pub fn repl(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            prompt: Some(Cow::Borrowed(">>>")),
            hidden: false,
        }
    }

    /// Creates a REPL command continuation input with the `...` prompt.
    pub fn repl_continuation(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            prompt: Some(Cow::Borrowed("...")),
            hidden: false,
        }
    }

    /// Returns the prompt part of this input.
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }

    /// Marks this input as hidden (one that should not be displayed in the rendered transcript).
    #[must_use]
    pub fn hide(mut self) -> Self {
        self.hidden = true;
        self
    }
}

/// Returns the command part of the input without the prompt.
impl AsRef<str> for UserInput {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

/// Calls [`Self::command()`] on the provided string reference.
impl From<&str> for UserInput {
    fn from(command: &str) -> Self {
        Self::command(command)
    }
}

#[cfg(doctest)]
doc_comment::doctest!("../README.md");
