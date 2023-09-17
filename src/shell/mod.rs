//! Shell-related types.

use std::{
    convert::Infallible,
    env, error,
    ffi::OsStr,
    fmt, io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

mod standard;
mod transcript_impl;

pub use self::standard::StdShell;

use crate::{
    traits::{ConfigureCommand, Echoing, SpawnShell, SpawnedShell},
    Captured, ExitStatus,
};

type StatusCheckerFn = dyn Fn(&Captured) -> Option<ExitStatus>;

pub(crate) struct StatusCheck {
    command: String,
    response_checker: Box<StatusCheckerFn>,
}

impl fmt::Debug for StatusCheck {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StatusCheck")
            .field("command", &self.command)
            .finish_non_exhaustive()
    }
}

impl StatusCheck {
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn check(&self, response: &Captured) -> Option<ExitStatus> {
        (self.response_checker)(response)
    }
}

/// Options for executing commands in the shell. Used in [`Transcript::from_inputs()`]
/// and in [`TestConfig`].
///
/// The type param maps to *extensions* available for the shell. For example, [`StdShell`]
/// extension allows to specify custom aliases for the executed commands.
///
/// [`TestConfig`]: crate::test::TestConfig
/// [`Transcript::from_inputs()`]: crate::Transcript::from_inputs()
pub struct ShellOptions<Cmd = Command> {
    command: Cmd,
    path_additions: Vec<PathBuf>,
    io_timeout: Duration,
    init_timeout: Duration,
    init_commands: Vec<String>,
    line_decoder: Box<dyn FnMut(Vec<u8>) -> io::Result<String>>,
    status_check: Option<StatusCheck>,
}

impl<Cmd: fmt::Debug> fmt::Debug for ShellOptions<Cmd> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShellOptions")
            .field("command", &self.command)
            .field("path_additions", &self.path_additions)
            .field("io_timeout", &self.io_timeout)
            .field("init_timeout", &self.init_timeout)
            .field("init_commands", &self.init_commands)
            .field("status_check", &self.status_check)
            .finish_non_exhaustive()
    }
}

#[cfg(any(unix, windows))]
impl Default for ShellOptions {
    fn default() -> Self {
        Self::new(Self::default_shell())
    }
}

impl<Cmd: ConfigureCommand> From<Cmd> for ShellOptions<Cmd> {
    fn from(command: Cmd) -> Self {
        Self::new(command)
    }
}

impl<Cmd: ConfigureCommand> ShellOptions<Cmd> {
    #[cfg(unix)]
    fn default_shell() -> Command {
        Command::new("sh")
    }

    #[cfg(windows)]
    fn default_shell() -> Command {
        let mut command = Command::new("cmd");
        // Switch off echoing user inputs and switch the codepage to UTF-8.
        command.arg("/Q").arg("/K").arg("echo off && chcp 65001");
        command
    }

    /// Creates new options with the provided `command`.
    pub fn new(command: Cmd) -> Self {
        Self {
            command,
            path_additions: vec![],
            io_timeout: Duration::from_millis(500),
            init_timeout: Duration::from_millis(1_500),
            init_commands: vec![],
            line_decoder: Box::new(|line| {
                String::from_utf8(line)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.utf8_error()))
            }),
            status_check: None,
        }
    }

    /// Sets the echoing flag for the shell.
    #[must_use]
    pub fn echoing(self, is_echoing: bool) -> ShellOptions<Echoing<Cmd>> {
        ShellOptions {
            command: Echoing::new(self.command, is_echoing),
            path_additions: self.path_additions,
            io_timeout: self.io_timeout,
            init_timeout: self.init_timeout,
            init_commands: self.init_commands,
            line_decoder: self.line_decoder,
            status_check: self.status_check,
        }
    }

    /// Changes the current directory of the command.
    #[must_use]
    pub fn with_current_dir(mut self, current_dir: impl AsRef<Path>) -> Self {
        self.command.current_dir(current_dir.as_ref());
        self
    }

    /// Sets the I/O timeout for shell commands. This determines how long methods like
    /// [`Transcript::from_inputs()`] wait
    /// for output of a command before proceeding to the next command. Longer values
    /// allow to capture output more accurately, but result in longer execution.
    ///
    /// By default, the I/O timeout is 0.5 seconds.
    ///
    /// [`Transcript::from_inputs()`]: crate::Transcript::from_inputs()
    #[must_use]
    pub fn with_io_timeout(mut self, io_timeout: Duration) -> Self {
        self.io_timeout = io_timeout;
        self
    }

    /// Sets an additional initialization timeout (relative to the one set by
    /// [`Self::with_io_timeout()`]) before reading the output of the each command
    /// input into the shell.
    ///
    /// By default, the initialization timeout is 1.5 seconds.
    #[must_use]
    pub fn with_init_timeout(mut self, init_timeout: Duration) -> Self {
        self.init_timeout = init_timeout;
        self
    }

    /// Adds an initialization command. Such commands are sent to the shell before executing
    /// any user input. The corresponding output from the shell is not captured.
    #[must_use]
    pub fn with_init_command(mut self, command: impl Into<String>) -> Self {
        self.init_commands.push(command.into());
        self
    }

    /// Sets the `value` of an environment variable with the specified `name`.
    #[must_use]
    pub fn with_env(mut self, name: impl AsRef<str>, value: impl AsRef<OsStr>) -> Self {
        self.command.env(name.as_ref(), value.as_ref());
        self
    }

    /// Sets the line decoder for the shell. This allows for custom shell text encodings.
    ///
    /// The default decoder used is [the UTF-8 one](String::from_utf8()).
    /// It halts processing with an error if the input is not UTF-8;
    /// you may use [`Self::with_lossy_utf8_decoder()`] to swallow errors in this case.
    #[must_use]
    pub fn with_line_decoder<E, F>(mut self, mut mapper: F) -> Self
    where
        E: Into<Box<dyn error::Error + Send + Sync>>,
        F: FnMut(Vec<u8>) -> Result<String, E> + 'static,
    {
        self.line_decoder = Box::new(move |line| {
            mapper(line).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        });
        self
    }

    /// Sets the [lossy UTF-8 decoder](String::from_utf8_lossy()) which always succeeds
    /// at decoding at the cost of replacing non-UTF-8 chars.
    #[must_use]
    pub fn with_lossy_utf8_decoder(self) -> Self {
        self.with_line_decoder::<Infallible, _>(|line| {
            Ok(String::from_utf8_lossy(&line).into_owned())
        })
    }

    /// Sets the [`ExitStatus`] checker for the shell. See `ExitStatus` docs for the semantics
    /// of exit statuses.
    ///
    /// # Arguments
    ///
    /// - `command` is a command that will be executed to check the exit status of the latest
    ///   executed command. E.g., in `sh`-like shells one can use `echo $?`.
    /// - `checker` is a closure that transforms the output of `command` into an `ExitStatus`.
    ///   The output is provided as a [`Captured`] string; it usually makes sense to use
    ///   [`Captured::to_plaintext()`] to strip it of possible escape sequences (especially
    ///   important if captured from PTY). If the exit status is inconclusive or not applicable,
    ///   the closure should return `None`.
    ///
    /// The `command` will be executed after each [`UserInput`] is input into the shell and
    /// the corresponding output is captured. After this, the [`Captured`]
    /// output will be supplied to the `checker` closure and its output will be recorded as
    /// [`Interaction::exit_status()`].
    ///
    /// [`UserInput`]: crate::UserInput
    /// [`Interaction::exit_status()`]: crate::Interaction::exit_status()
    ///
    /// # Panics
    ///
    /// Panics if `command` contains newline chars (`'\n'` or `'\r'`).
    #[must_use]
    pub fn with_status_check<F>(mut self, command: impl Into<String>, checker: F) -> Self
    where
        F: Fn(&Captured) -> Option<ExitStatus> + 'static,
    {
        let command = command.into();
        assert!(
            command.bytes().all(|ch| ch != b'\n' && ch != b'\r'),
            "`command` contains a newline character ('\\n' or '\\r')"
        );

        self.status_check = Some(StatusCheck {
            command,
            response_checker: Box::new(checker),
        });
        self
    }

    // Gets the path to the cargo `target` dir. Adapted from cargo:
    //
    // https://github.com/rust-lang/cargo/blob/485670b3983b52289a2f353d589c57fae2f60f82/tests/testsuite/support/mod.rs#L507
    fn target_path() -> PathBuf {
        let mut path = env::current_exe().expect("Cannot obtain path to the executing file");
        path.pop();
        if path.ends_with("deps") {
            path.pop();
        }
        path
    }

    /// Adds paths to cargo binaries (including examples) to the `PATH` env variable
    /// for the shell described by these options.
    /// This allows to call them by the corresponding filename, without specifying a path
    /// or doing complex preparations (e.g., calling `cargo install`).
    ///
    /// # Limitations
    ///
    /// - The caller must be a unit or integration test; the method will work improperly otherwise.
    #[must_use]
    pub fn with_cargo_path(mut self) -> Self {
        let target_path = Self::target_path();
        self.path_additions.push(target_path.join("examples"));
        self.path_additions.push(target_path);
        self
    }

    /// Adds a specified path to the `PATH` env variable for the shell described by these options.
    /// This method can be called multiple times to add multiple paths and is composable
    /// with [`Self::with_cargo_path()`].
    #[must_use]
    pub fn with_additional_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        self.path_additions.push(path);
        self
    }
}

impl<Cmd: SpawnShell> ShellOptions<Cmd> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "debug",
            skip(self),
            err,
            fields(self.path_additions = ?self.path_additions)
        )
    )]
    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Cmd>> {
        #[cfg(unix)]
        const PATH_SEPARATOR: &str = ":";
        #[cfg(windows)]
        const PATH_SEPARATOR: &str = ";";

        if !self.path_additions.is_empty() {
            let mut path_var = env::var_os("PATH").unwrap_or_default();
            if !path_var.is_empty() {
                path_var.push(PATH_SEPARATOR);
            }
            for (i, addition) in self.path_additions.iter().enumerate() {
                path_var.push(addition);
                if i + 1 < self.path_additions.len() {
                    path_var.push(PATH_SEPARATOR);
                }
            }
            self.command.env("PATH", &path_var);
        }
        self.command.spawn_shell()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Transcript, UserInput};

    #[cfg(any(unix, windows))]
    #[test]
    fn creating_transcript_basics() -> anyhow::Result<()> {
        let inputs = vec![
            UserInput::command("echo hello"),
            UserInput::command("echo foo && echo bar >&2"),
        ];
        let transcript = Transcript::from_inputs(&mut ShellOptions::default(), inputs)?;

        assert_eq!(transcript.interactions().len(), 2);

        {
            let interaction = &transcript.interactions()[0];
            assert_eq!(interaction.input().text, "echo hello");
            let output = interaction.output().as_ref();
            assert_eq!(output.trim(), "hello");
        }

        let interaction = &transcript.interactions()[1];
        assert_eq!(interaction.input().text, "echo foo && echo bar >&2");
        let output = interaction.output().as_ref();
        assert_eq!(
            output.split_whitespace().collect::<Vec<_>>(),
            ["foo", "bar"]
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn transcript_with_multiline_input() -> anyhow::Result<()> {
        let mut options = ShellOptions::default();
        let inputs = vec![UserInput::command("echo \\\nhello")];
        let transcript = Transcript::from_inputs(&mut options, inputs)?;

        assert_eq!(transcript.interactions().len(), 1);
        let interaction = &transcript.interactions()[0];
        let output = interaction.output().as_ref();
        assert_eq!(output.trim(), "hello");
        Ok(())
    }
}
