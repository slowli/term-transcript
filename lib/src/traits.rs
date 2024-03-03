//! Traits for interaction with the terminal.

use std::{
    ffi::OsStr,
    io,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

use crate::utils::is_recoverable_kill_error;

/// Common denominator for types that can be used to configure commands for
/// execution in the terminal.
pub trait ConfigureCommand {
    /// Sets the current directory.
    fn current_dir(&mut self, dir: &Path);
    /// Sets an environment variable.
    fn env(&mut self, name: &str, value: &OsStr);
}

impl ConfigureCommand for Command {
    fn current_dir(&mut self, dir: &Path) {
        self.current_dir(dir);
    }

    fn env(&mut self, name: &str, value: &OsStr) {
        self.env(name, value);
    }
}

/// Encapsulates spawning and sending inputs / receiving outputs from the shell.
///
/// The crate provides two principal implementations of this trait:
///
/// - [`Command`] and [`StdShell`](crate::StdShell) communicate with the spawned process
///   via OS pipes. Because stdin of the child process is not connected to a terminal / TTY,
///   this can lead to the differences in output compared to launching the process in a terminal
///   (no coloring, different formatting, etc.). On the other hand, this is the most widely
///   supported option.
/// - [`PtyCommand`](crate::PtyCommand) (available with the `portable-pty` crate feature)
///   communicates with the child process via a pseudo-terminal (PTY). This makes the output
///   closer to what it would like in the terminal, at the cost of lesser platform coverage
///   (Unix + newer Windows distributions).
///
/// External implementations are possible as well! E.g., for REPL applications written in Rust
/// or packaged as a [WASI] module, it could be possible to write an implementation that "spawns"
/// the application in the same process.
///
/// [WASI]: https://wasi.dev/
pub trait SpawnShell: ConfigureCommand {
    /// Spawned shell process.
    type ShellProcess: ShellProcess;
    /// Reader of the shell output.
    type Reader: io::Read + 'static + Send;
    /// Writer to the shell input.
    type Writer: io::Write + 'static + Send;

    /// Spawns a shell process.
    ///
    /// # Errors
    ///
    /// Returns an error if the shell process cannot be spawned for whatever reason.
    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>>;
}

/// Representation of a shell process.
pub trait ShellProcess {
    /// Checks if the process is alive.
    ///
    /// # Errors
    ///
    /// Returns an error if the process is not alive. Should include debug details if possible
    /// (e.g., the exit status of the process).
    fn check_is_alive(&mut self) -> io::Result<()>;

    /// Terminates the shell process. This can include killing it if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be killed.
    fn terminate(self) -> io::Result<()>;

    /// Returns `true` if the input commands are echoed back to the output.
    ///
    /// The default implementation returns `false`.
    fn is_echoing(&self) -> bool {
        false
    }
}

/// Wrapper for spawned shell and related I/O returned by [`SpawnShell::spawn_shell()`].
#[derive(Debug)]
pub struct SpawnedShell<S: SpawnShell + ?Sized> {
    /// Shell process.
    pub shell: S::ShellProcess,
    /// Reader of shell output.
    pub reader: S::Reader,
    /// Writer to shell input.
    pub writer: S::Writer,
}

/// Uses pipes to communicate with the spawned process. This has a potential downside that
/// the process output will differ from the case if the process was launched in the shell.
/// See [`PtyCommand`] for an alternative that connects the spawned process to a pseudo-terminal
/// (PTY).
///
/// [`PtyCommand`]: crate::PtyCommand
impl SpawnShell for Command {
    type ShellProcess = Child;
    type Reader = os_pipe::PipeReader;
    type Writer = ChildStdin;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", err))]
    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
        #[cfg(feature = "tracing")]
        tracing::debug!("created OS pipe");

        let mut shell = self
            .stdin(Stdio::piped())
            .stdout(pipe_writer.try_clone()?)
            .stderr(pipe_writer)
            .spawn()?;
        #[cfg(feature = "tracing")]
        tracing::debug!("created child");

        self.stdout(Stdio::null()).stderr(Stdio::null());

        let stdin = shell.stdin.take().unwrap();
        // ^-- `unwrap()` is safe due to configuration of the shell process.

        Ok(SpawnedShell {
            shell,
            reader: pipe_reader,
            writer: stdin,
        })
    }
}

impl ShellProcess for Child {
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", err))]
    fn check_is_alive(&mut self) -> io::Result<()> {
        if let Some(exit_status) = self.try_wait()? {
            let message = format!("Shell process has prematurely exited: {exit_status}");
            Err(io::Error::new(io::ErrorKind::BrokenPipe, message))
        } else {
            Ok(())
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", err))]
    fn terminate(mut self) -> io::Result<()> {
        if self.try_wait()?.is_none() {
            self.kill().or_else(|err| {
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

/// Wrapper that allows configuring echoing of the shell process.
///
/// A shell process is echoing if each line provided to the input is echoed to the output.
#[derive(Debug, Clone)]
pub struct Echoing<S> {
    inner: S,
    is_echoing: bool,
}

impl<S> Echoing<S> {
    /// Wraps the provided `inner` type.
    pub fn new(inner: S, is_echoing: bool) -> Self {
        Self { inner, is_echoing }
    }
}

impl<S: ConfigureCommand> ConfigureCommand for Echoing<S> {
    fn current_dir(&mut self, dir: &Path) {
        self.inner.current_dir(dir);
    }

    fn env(&mut self, name: &str, value: &OsStr) {
        self.inner.env(name, value);
    }
}

impl<S: SpawnShell> SpawnShell for Echoing<S> {
    type ShellProcess = Echoing<S::ShellProcess>;
    type Reader = S::Reader;
    type Writer = S::Writer;

    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let spawned = self.inner.spawn_shell()?;
        Ok(SpawnedShell {
            shell: Echoing {
                inner: spawned.shell,
                is_echoing: self.is_echoing,
            },
            reader: spawned.reader,
            writer: spawned.writer,
        })
    }
}

impl<S: ShellProcess> ShellProcess for Echoing<S> {
    fn check_is_alive(&mut self) -> io::Result<()> {
        self.inner.check_is_alive()
    }

    fn terminate(self) -> io::Result<()> {
        self.inner.terminate()
    }

    fn is_echoing(&self) -> bool {
        self.is_echoing
    }
}
