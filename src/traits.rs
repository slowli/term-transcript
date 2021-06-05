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
pub trait SpawnShell {
    /// Spawned shell process.
    type ShellProcess: ShellProcess;
    /// Reader of shell output.
    type Reader: io::Read + 'static + Send;
    /// Writer to shell input.
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
    /// Returns `true` if the input commands are echoed back to the output.
    fn is_echoing(&self) -> bool;
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

impl SpawnShell for Command {
    type ShellProcess = ChildShell;
    type Reader = os_pipe::PipeReader;
    type Writer = ChildStdin;

    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
        let mut shell = self
            .stdin(Stdio::piped())
            .stdout(pipe_writer.try_clone()?)
            .stderr(pipe_writer)
            .spawn()?;

        self.stdout(Stdio::null()).stderr(Stdio::null());

        let stdin = shell.stdin.take().unwrap();
        // ^-- `unwrap()` is safe due to configuration of the shell process.

        Ok(SpawnedShell {
            shell: ChildShell::new(shell, false),
            reader: pipe_reader,
            writer: stdin,
        })
    }
}

/// [`ShellProcess`] implementation based on [`Child`] from the Rust standard library.
#[derive(Debug)]
pub struct ChildShell {
    child: Child,
    is_echoing: bool,
}

impl ChildShell {
    /// Creates a `ChildShell` instance based on the `child` process and an indicator
    /// whether it is echoing.
    pub fn new(child: Child, is_echoing: bool) -> Self {
        Self { child, is_echoing }
    }

    pub(crate) fn set_echoing(&mut self) {
        self.is_echoing = true;
    }
}

impl ShellProcess for ChildShell {
    fn is_echoing(&self) -> bool {
        self.is_echoing
    }

    fn check_is_alive(&mut self) -> io::Result<()> {
        if let Some(exit_status) = self.child.try_wait()? {
            let message = format!(
                "Shell process has prematurely exited with exit status {}",
                exit_status
            );
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
