//! Shell-related types.

use std::{
    env,
    ffi::OsStr,
    fmt, io,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

mod standard;
mod transcript_impl;

pub use self::standard::StdShell;

use crate::traits::{ConfigureCommand, SpawnShell, SpawnedShell};

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
    line_mapper: Box<dyn FnMut(String) -> Option<String>>,
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
            .finish()
    }
}

#[cfg(any(unix, windows))]
impl Default for ShellOptions {
    fn default() -> Self {
        Self::new(Self::default_shell())
    }
}

impl From<Command> for ShellOptions {
    fn from(command: Command) -> Self {
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
            io_timeout: Duration::from_secs(1),
            init_timeout: Duration::from_nanos(0),
            init_commands: vec![],
            line_mapper: Box::new(Some),
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
    /// By default, the I/O timeout is 1 second.
    ///
    /// [`Transcript::from_inputs()`]: crate::Transcript::from_inputs()
    #[must_use]
    pub fn with_io_timeout(mut self, io_timeout: Duration) -> Self {
        self.io_timeout = io_timeout;
        self
    }

    /// Sets an additional initialization timeout (relative to the one set by
    /// [`Self::with_io_timeout()`]) before reading the output of the first command.
    ///
    /// By default, the initialization timeout is zero.
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

    /// Sets the line mapper for the shell. This allows to filter and/or map terminal outputs.
    #[must_use]
    pub fn with_line_mapper<F>(mut self, mapper: F) -> Self
    where
        F: FnMut(String) -> Option<String> + 'static,
    {
        self.line_mapper = Box::new(mapper);
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
