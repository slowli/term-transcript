//! Core type definitions.

use std::{borrow::Cow, fmt, io};

use styled_str::{AnsiError, StyledString};

pub(crate) type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that can occur when processing terminal output.
#[derive(Debug)]
#[non_exhaustive]
pub enum TermError {
    /// Ansi escape sequence parsing error.
    Ansi(AnsiError),
    /// IO error.
    Io(io::Error),
    /// Font embedding error.
    FontEmbedding(BoxedError),
}

impl fmt::Display for TermError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ansi(err) => write!(formatter, "ANSI escape sequence parsing error: {err}"),
            Self::Io(err) => write!(formatter, "I/O error: {err}"),
            Self::FontEmbedding(err) => write!(formatter, "font embedding error: {err}"),
        }
    }
}

impl std::error::Error for TermError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ansi(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

/// Transcript of a user interacting with the terminal.
#[derive(Debug, Clone, Default)]
pub struct Transcript {
    interactions: Vec<Interaction>,
}

impl Transcript {
    /// Creates an empty transcript.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns interactions in this transcript.
    pub fn interactions(&self) -> &[Interaction] {
        &self.interactions
    }

    /// Returns a mutable reference to interactions in this transcript.
    pub fn interactions_mut(&mut self) -> &mut [Interaction] {
        &mut self.interactions
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
    /// This is a shortcut for calling [`Self::add_existing_interaction()`].
    pub fn add_interaction(
        &mut self,
        input: impl Into<UserInput>,
        output: StyledString,
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
pub struct Interaction {
    input: UserInput,
    output: StyledString,
    exit_status: Option<ExitStatus>,
}

impl Interaction {
    /// Creates a new interaction.
    ///
    /// Any newlines at the end of the output will be trimmed.
    pub fn new(input: impl Into<UserInput>, mut output: StyledString) -> Self {
        while output.text().ends_with('\n') {
            output.pop();
        }

        Self {
            input: input.into(),
            output,
            exit_status: None,
        }
    }

    /// Sets an exit status for this interaction.
    pub fn set_exit_status(&mut self, exit_status: Option<ExitStatus>) {
        self.exit_status = exit_status;
    }

    /// Assigns an exit status to this interaction.
    #[must_use]
    pub fn with_exit_status(mut self, exit_status: ExitStatus) -> Self {
        self.exit_status = Some(exit_status);
        self
    }
}

impl Interaction {
    /// Input provided by the user.
    pub fn input(&self) -> &UserInput {
        &self.input
    }

    /// Output to the terminal.
    pub fn output(&self) -> &StyledString {
        &self.output
    }

    /// Sets the output for this interaction.
    pub fn set_output(&mut self, output: StyledString) {
        self.output = output;
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
    pub(crate) const EMPTY: Self = Self {
        text: String::new(),
        prompt: None,
        hidden: false,
    };

    #[cfg(feature = "test")]
    pub(crate) fn new(text: String) -> Self {
        Self {
            prompt: None,
            text,
            hidden: false,
        }
    }

    #[must_use]
    pub(crate) fn with_prompt(mut self, prompt: Option<String>) -> Self {
        self.prompt = prompt.map(|prompt| match prompt.as_str() {
            "$" => Cow::Borrowed("$"),
            ">>>" => Cow::Borrowed(">>>"),
            "..." => Cow::Borrowed("..."),
            _ => Cow::Owned(prompt),
        });
        self
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

    /// Checks whether this input is hidden.
    pub fn is_hidden(&self) -> bool {
        self.hidden
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
