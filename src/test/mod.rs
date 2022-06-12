//! Snapshot testing tools for [`Transcript`](crate::Transcript)s.
//!
//! # Examples
//!
//! Simple scenario in which the tested transcript calls to one or more Cargo binaries / examples
//! by their original names.
//!
//! ```
//! use term_transcript::{
//!     ShellOptions, Transcript,
//!     test::{MatchKind, TestConfig, TestOutputConfig},
//! };
//!
//! // Test configuration that can be shared across tests.
//! fn config() -> TestConfig {
//!     let shell_options = ShellOptions::default().with_cargo_path();
//!     TestConfig::new(shell_options)
//!         .with_match_kind(MatchKind::Precise)
//!         .with_output(TestOutputConfig::Verbose)
//! }
//!
//! // Usage in tests:
//! #[test]
//! fn help_command() {
//!     config().test("tests/__snapshots__/help.svg", &["my-command --help"]);
//! }
//! ```
//!
//! Use [`TestConfig::test_transcript()`] for more complex scenarios or increased control:
//!
//! ```
//! use term_transcript::{test::TestConfig, ShellOptions, Transcript, UserInput};
//! # use term_transcript::svg::{Template, TemplateOptions};
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

use termcolor::ColorChoice;

#[cfg(feature = "svg")]
use std::ffi::OsStr;
use std::{env, process::Command, str};

mod color_diff;
mod config_impl;
mod parser;
#[cfg(test)]
mod tests;
mod utils;

pub use self::parser::Parsed;

#[cfg(feature = "svg")]
use crate::svg::Template;
use crate::{traits::SpawnShell, ShellOptions};

/// Configuration of output produced during testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TestOutputConfig {
    /// Do not output anything.
    Quiet,
    /// Output normal amount of details.
    Normal,
    /// Output more details.
    Verbose,
}

impl Default for TestOutputConfig {
    fn default() -> Self {
        Self::Normal
    }
}

/// Strategy for saving a new snapshot on a test failure within [`TestConfig::test()`] and
/// related methods.
#[derive(Debug, Clone, Copy, PartialEq, Hash)]
#[non_exhaustive]
#[cfg(feature = "svg")]
#[cfg_attr(docsrs, doc(cfg(feature = "svg")))]
pub enum UpdateMode {
    /// Never create a new snapshot on test failure.
    Never,
    /// Always create a new snapshot on test failure.
    Always,
}

#[cfg(feature = "svg")]
impl UpdateMode {
    /// Reads the update mode from the `TERM_TRANSCRIPT_UPDATE` env variable.
    ///
    /// If the `TERM_TRANSCRIPT_UPDATE` variable is not set, the output depends on whether
    /// the executable is running in CI (which is detected by the presence of
    /// the `CI` env variable):
    ///
    /// - In CI, the method returns [`Self::Never`].
    /// - Otherwise, the method returns [`Self::Always`].
    ///
    /// # Panics
    ///
    /// If the `TERM_TRANSCRIPT_UPDATE` env variable is set to an unrecognized value
    /// (something other than `never` or `always`), this method will panic.
    pub fn from_env() -> Self {
        const ENV_VAR: &str = "TERM_TRANSCRIPT_UPDATE";

        match env::var_os(ENV_VAR) {
            Some(s) => Self::from_os_str(&s).unwrap_or_else(|| {
                panic!(
                    "Cannot read update mode from env variable {}: `{}` is not a valid value \
                     (use one of `never` or `always`)",
                    ENV_VAR,
                    s.to_string_lossy()
                );
            }),
            None => {
                if env::var_os("CI").is_some() {
                    Self::Never
                } else {
                    Self::Always
                }
            }
        }
    }

    fn from_os_str(s: &OsStr) -> Option<Self> {
        match s {
            s if s == "never" => Some(Self::Never),
            s if s == "always" => Some(Self::Always),
            _ => None,
        }
    }

    fn should_create_snapshot(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// Testing configuration.
///
/// # Examples
///
/// See the [module docs](crate::test) for the examples of usage.
#[derive(Debug)]
pub struct TestConfig<Cmd = Command> {
    shell_options: ShellOptions<Cmd>,
    match_kind: MatchKind,
    output: TestOutputConfig,
    color_choice: ColorChoice,
    #[cfg(feature = "svg")]
    update_mode: UpdateMode,
    #[cfg(feature = "svg")]
    template: Template,
}

impl<Cmd: SpawnShell> TestConfig<Cmd> {
    /// Creates a new config.
    ///
    /// # Panics
    ///
    /// - Panics if the `svg` crate feature is enabled and the `TERM_TRANSCRIPT_UPDATE` variable
    ///   is set to an incorrect value. See [`UpdateMode::from_env()`] for more details.
    pub fn new(shell_options: ShellOptions<Cmd>) -> Self {
        Self {
            shell_options,
            match_kind: MatchKind::TextOnly,
            output: TestOutputConfig::Normal,
            color_choice: ColorChoice::Auto,
            #[cfg(feature = "svg")]
            update_mode: UpdateMode::from_env(),
            #[cfg(feature = "svg")]
            template: Template::default(),
        }
    }

    /// Sets the matching kind applied.
    #[must_use]
    pub fn with_match_kind(mut self, kind: MatchKind) -> Self {
        self.match_kind = kind;
        self
    }

    /// Sets coloring of the output.
    ///
    /// On Windows, `color_choice` has slightly different semantics than its usage
    /// in the `termcolor` crate. Namely, if colors can be used (stdout is a tty with
    /// color support), ANSI escape sequences will always be used.
    #[must_use]
    pub fn with_color_choice(mut self, color_choice: ColorChoice) -> Self {
        self.color_choice = color_choice;
        self
    }

    /// Configures test output.
    #[must_use]
    pub fn with_output(mut self, output: TestOutputConfig) -> Self {
        self.output = output;
        self
    }

    /// Sets the template for rendering new snapshots.
    #[cfg(feature = "svg")]
    #[cfg_attr(docsrs, doc(cfg(feature = "svg")))]
    #[must_use]
    pub fn with_template(mut self, template: Template) -> Self {
        self.template = template;
        self
    }

    /// Overrides the strategy for saving new snapshots for failed tests.
    ///
    /// By default, the strategy is determined from the execution environment
    /// using [`UpdateMode::from_env()`].
    #[cfg(feature = "svg")]
    #[cfg_attr(docsrs, doc(cfg(feature = "svg")))]
    #[must_use]
    pub fn with_update_mode(mut self, update_mode: UpdateMode) -> Self {
        self.update_mode = update_mode;
        self
    }
}

/// Kind of terminal output matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum MatchKind {
    /// Relaxed matching: compare only output text, but not coloring.
    TextOnly,
    /// Precise matching: compare output together with colors.
    Precise,
}

/// Stats of a single snapshot test output by [`TestConfig::test_transcript_for_stats()`].
#[derive(Debug, Clone)]
pub struct TestStats {
    // Match kind per each user input.
    matches: Vec<Option<MatchKind>>,
}

impl TestStats {
    /// Returns the number of successfully matched user inputs with at least the specified
    /// `match_level`.
    pub fn passed(&self, match_level: MatchKind) -> usize {
        self.matches
            .iter()
            .filter(|&&kind| kind >= Some(match_level))
            .count()
    }

    /// Returns the number of user inputs that do not match with at least the specified
    /// `match_level`.
    pub fn errors(&self, match_level: MatchKind) -> usize {
        self.matches.len() - self.passed(match_level)
    }

    /// Returns match kinds per each user input of the tested [`Transcript`]. `None` values
    /// mean no match.
    ///
    /// [`Transcript`]: crate::Transcript
    pub fn matches(&self) -> &[Option<MatchKind>] {
        &self.matches
    }

    /// Panics if these stats contain errors.
    #[allow(clippy::missing_panics_doc)]
    pub fn assert_no_errors(&self, match_level: MatchKind) {
        assert_eq!(self.errors(match_level), 0, "There were test errors");
    }
}
