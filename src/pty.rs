//! Spawning shell in PTY via `portable-pty` crate.

// FIXME: Prompt incorrectly read from PTY in some cases (#24)

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtyPair, PtySize};

use std::{
    collections::HashMap,
    error::Error as StdError,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
};

use crate::{
    traits::{ConfigureCommand, ShellProcess, SpawnShell, SpawnedShell},
    utils::is_recoverable_kill_error,
};

fn into_io_error(err: Box<dyn StdError + Send + Sync>) -> io::Error {
    err.downcast::<io::Error>()
        .map_or_else(|err| io::Error::new(io::ErrorKind::Other, err), |err| *err)
}

/// Command to spawn in a pseudo-terminal (PTY).
///
/// # Examples
///
/// Since shell spawning is performed [in a generic way](crate::traits::SpawnShell),
/// [`PtyCommand`] can be used as a drop-in replacement for [`Command`](std::process::Command):
///
/// ```
/// # use term_transcript::{PtyCommand, ShellOptions, UserInput, Transcript};
/// # fn main() -> anyhow::Result<()> {
/// let transcript = Transcript::from_inputs(
///     &mut ShellOptions::new(PtyCommand::default()),
///     vec![UserInput::command(r#"echo "Hello world!""#)],
/// )?;
/// // do something with `transcript`...
/// # Ok(())
/// # }
/// ```
// Unfortunately, the `portable-pty` is structured in a way that makes reusing `Command`
// from the standard library impossible.
#[cfg_attr(docsrs, doc(cfg(feature = "portable-pty")))]
#[derive(Debug, Clone)]
pub struct PtyCommand {
    args: Vec<OsString>,
    env: HashMap<OsString, OsString>,
    current_dir: Option<PathBuf>,
    pty_size: PtySize,
}

#[cfg(unix)]
impl Default for PtyCommand {
    fn default() -> Self {
        Self::new("sh")
    }
}

#[cfg(windows)]
impl Default for PtyCommand {
    fn default() -> Self {
        let mut cmd = Self::new("cmd");
        cmd.arg("/Q").arg("/K").arg("echo off && chcp 65001");
        cmd
    }
}

impl PtyCommand {
    /// Creates a new command based on the executable.
    ///
    /// This uses a reasonable default for the PTY size (19 character rows, 80 columns).
    pub fn new(command: impl Into<OsString>) -> Self {
        Self {
            args: vec![command.into()],
            env: HashMap::new(),
            current_dir: None,
            pty_size: PtySize {
                rows: 19,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        }
    }

    /// Sets the size of the PTY in characters.
    pub fn with_size(&mut self, rows: u16, cols: u16) -> &mut Self {
        self.pty_size.rows = rows;
        self.pty_size.cols = cols;
        self
    }

    /// Adds a command argument.
    pub fn arg(&mut self, arg: impl Into<OsString>) -> &mut Self {
        self.args.push(arg.into());
        self
    }

    fn to_command_builder(&self) -> CommandBuilder {
        let mut builder = CommandBuilder::from_argv(self.args.clone());
        for (name, value) in &self.env {
            builder.env(name, value);
        }
        if let Some(current_dir) = &self.current_dir {
            builder.cwd(current_dir);
        }
        builder
    }
}

impl ConfigureCommand for PtyCommand {
    fn current_dir(&mut self, dir: &Path) {
        self.current_dir = Some(dir.to_owned());
    }

    fn env(&mut self, name: &str, value: &OsStr) {
        self.env
            .insert(OsStr::new(name).to_owned(), value.to_owned());
    }
}

impl SpawnShell for PtyCommand {
    type ShellProcess = PtyShell;
    type Reader = Box<dyn io::Read + Send>;
    type Writer = Box<dyn MasterPty + Send>;

    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let pty_system = native_pty_system();
        let PtyPair { master, slave } = pty_system
            .openpty(self.pty_size)
            .map_err(|err| into_io_error(err.into()))?;

        let child = slave
            .spawn_command(self.to_command_builder())
            .map_err(|err| into_io_error(err.into()))?;

        let reader = master
            .try_clone_reader()
            .map_err(|err| into_io_error(err.into()))?;

        Ok(SpawnedShell {
            shell: PtyShell { child },
            reader,
            writer: master,
        })
    }
}

/// Spawned shell process connected to pseudo-terminal (PTY).
#[cfg_attr(docsrs, doc(cfg(feature = "portable-pty")))]
#[derive(Debug)]
pub struct PtyShell {
    child: Box<dyn Child + Send + Sync>,
}

impl ShellProcess for PtyShell {
    fn is_echoing(&self) -> bool {
        true
    }

    fn check_is_alive(&mut self) -> io::Result<()> {
        if let Some(exit_status) = self.child.try_wait()? {
            let status_str = if exit_status.success() {
                "zero"
            } else {
                "non-zero"
            };
            let message =
                format!("Shell process has prematurely exited with {status_str} exit status");
            Err(io::Error::new(io::ErrorKind::BrokenPipe, message))
        } else {
            Ok(())
        }
    }

    fn terminate(mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().or_else(|err| {
                if is_recoverable_kill_error(&err) {
                    // The shell has already exited. We don't consider this an error.
                    Ok(())
                } else {
                    Err(err)
                }
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ShellOptions, Transcript, UserInput};

    use std::{
        io::{Read, Write},
        thread,
        time::Duration,
    };

    #[test]
    fn pty_trait_implementation() -> anyhow::Result<()> {
        let mut pty_command = PtyCommand::default();
        let mut spawned = pty_command.spawn_shell()?;

        thread::sleep(Duration::from_millis(100));
        spawned.shell.check_is_alive()?;

        writeln!(spawned.writer, "echo Hello")?;
        thread::sleep(Duration::from_millis(100));
        spawned.shell.check_is_alive()?;

        drop(spawned.writer); // should be enough to terminate the shell
        thread::sleep(Duration::from_millis(100));

        spawned.shell.terminate()?;
        let mut buffer = String::new();
        spawned.reader.read_to_string(&mut buffer)?;

        assert!(buffer.contains("Hello"), "Unexpected buffer: {buffer:?}");
        Ok(())
    }

    #[test]
    fn creating_transcript_with_pty() -> anyhow::Result<()> {
        let mut options = ShellOptions::new(PtyCommand::default());
        let inputs = vec![
            UserInput::command("echo hello"),
            UserInput::command("echo foo && echo bar >&2"),
        ];
        let transcript = Transcript::from_inputs(&mut options, inputs)?;

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
    fn pty_transcript_with_multiline_input() -> anyhow::Result<()> {
        let mut options = ShellOptions::new(PtyCommand::default());
        let inputs = vec![UserInput::command("echo \\\nhello")];
        let transcript = Transcript::from_inputs(&mut options, inputs)?;

        assert_eq!(transcript.interactions().len(), 1);
        let interaction = &transcript.interactions()[0];
        let output = interaction.output().as_ref();
        assert_eq!(output.trim(), "hello");
        Ok(())
    }
}
