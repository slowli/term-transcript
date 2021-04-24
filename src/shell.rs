use std::{
    env, fmt,
    io::{self, BufRead, BufReader, LineWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::{Captured, Interaction, Transcript, UserInput};

/// Options for executing commands in the shell. Used in [`Transcript::from_inputs()`]
/// and in [`TestConfig`].
///
/// The type param maps to *extensions* available for the shell. For example, [`StdShell`]
/// extension allows to specify custom aliases for the executed commands.
///
/// [`TestConfig`]: crate::test::TestConfig
pub struct ShellOptions<Ext = ()> {
    command: Command,
    io_timeout: Duration,
    init_commands: Vec<String>,
    line_mapper: Box<dyn FnMut(String) -> Option<String>>,
    extensions: Ext,
}

impl<Ext: fmt::Debug> fmt::Debug for ShellOptions<Ext> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShellOptions")
            .field("command", &self.command)
            .field("io_timeout", &self.io_timeout)
            .field("init_commands", &self.init_commands)
            .field("extensions", &self.extensions)
            .finish()
    }
}

#[cfg(any(unix, windows))]
impl Default for ShellOptions {
    fn default() -> Self {
        Self::new(Self::default_shell(), ())
    }
}

impl From<Command> for ShellOptions {
    fn from(command: Command) -> Self {
        Self::new(command, ())
    }
}

impl<Ext> ShellOptions<Ext> {
    #[cfg(unix)]
    fn default_shell() -> Command {
        Command::new("sh")
    }

    #[cfg(windows)]
    fn default_shell() -> Command {
        let mut command = Command::new("cmd");
        command.arg("/Q").arg("/K").arg("echo off");
        command
    }

    fn new(command: Command, extensions: Ext) -> Self {
        Self {
            command,
            io_timeout: Duration::from_secs(1),
            init_commands: vec![],
            line_mapper: Box::new(Some),
            extensions,
        }
    }

    /// Changes the current directory of the command.
    pub fn with_current_dir(mut self, current_dir: impl AsRef<Path>) -> Self {
        self.command.current_dir(current_dir);
        self
    }

    /// Sets the I/O timeout for shell commands. This determines how long the code waits
    /// for output of a command before proceeding to the next command. Longer values
    /// allow to capture output more accurately, but result in longer execution.
    pub fn with_io_timeout(mut self, io_timeout: Duration) -> Self {
        self.io_timeout = io_timeout;
        self
    }

    /// Adds an initialization command. Such commands are sent to the shell before executing
    /// any user input. The corresponding output from the shell is not captured.
    pub fn with_init_command(mut self, command: impl Into<String>) -> Self {
        self.init_commands.push(command.into());
        self
    }

    /// Sets the line mapper for the shell. This allows to filter and/or map terminal outputs.
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

    /// Adds paths to cargo binaries (including examples) to the `PATH` env variable.
    /// This allows to call them by the corresponding filename, without specifying a path
    /// or doing complex preparations (e.g., calling `cargo install`).
    ///
    /// # Limitations
    ///
    /// - The caller must be a unit or integration test; the method will work improperly otherwise.
    #[cfg(any(unix, windows))]
    #[cfg_attr(docsrs, doc(cfg(any(unix, windows))))]
    pub fn with_cargo_path(mut self) -> Self {
        #[cfg(unix)]
        const PATH_SEPARATOR: &str = ":";
        #[cfg(windows)]
        const PATH_SEPARATOR: &str = ";";

        let mut path_var = env::var_os("PATH").unwrap_or_default();
        let target_path = Self::target_path();
        if !path_var.is_empty() {
            path_var.push(PATH_SEPARATOR);
        }
        path_var.push(target_path.join("examples"));
        path_var.push(PATH_SEPARATOR);
        path_var.push(target_path);

        self.command.env("PATH", &path_var);
        self
    }

    pub(crate) fn drop_extensions(self) -> ShellOptions {
        ShellOptions {
            command: self.command,
            io_timeout: self.io_timeout,
            init_commands: self.init_commands,
            line_mapper: self.line_mapper,
            extensions: (),
        }
    }
}

/// Shell interpreter that brings additional functionality for [`ShellOptions`].
#[derive(Debug)]
#[non_exhaustive]
pub enum StdShell {
    /// `sh` shell.
    Sh,
    /// `bash` shell.
    Bash,
    /// PowerShell.
    PowerShell,
}

impl ShellOptions<StdShell> {
    /// Creates options for an `sh` shell.
    pub fn sh() -> Self {
        Self::new(Command::new("sh"), StdShell::Sh)
    }

    /// Creates options for a Bash shell.
    pub fn bash() -> Self {
        Self::new(Command::new("bash"), StdShell::Bash)
    }

    /// Creates options for PowerShell.
    #[allow(clippy::doc_markdown)] // false positive
    pub fn powershell() -> Self {
        let mut cmd = Command::new("powershell");
        cmd.arg("-NoLogo").arg("-NoExit");

        Self::new(cmd, StdShell::PowerShell)
            .with_init_command("function prompt { }")
            .with_line_mapper(|line| {
                if line.starts_with("PS>") {
                    None
                } else {
                    Some(line)
                }
            })
    }

    /// Creates an alias for the specified cargo binary, such as `foo` or `examples/bar`.
    /// This allows to call the binary using this alias without complex preparations (such as
    /// installing it globally via `cargo install`), and is more flexible than
    /// [`Self::with_cargo_path()`].
    ///
    /// # Limitations
    ///
    /// - The caller must be a unit or integration test; the method will work improperly otherwise.
    /// - For Bash and PowerShell, `name` must be a valid name of a function. For `sh`,
    ///   `name` must be a valid name for the `alias` command. The `name` validity
    ///   is **not** checked.
    #[allow(clippy::doc_markdown)] // false positive
    pub fn with_alias(self, name: &str, path_to_bin: impl AsRef<Path>) -> Self {
        let path_to_bin = Self::cargo_bin(path_to_bin);
        let path_to_bin = path_to_bin
            .to_str()
            .expect("Path to example is not a UTF-8 string");

        let alias_command = match self.extensions {
            StdShell::Sh => format!("alias {name}=\"'{path}'\"", name = name, path = path_to_bin),
            StdShell::Bash => format!(
                "{name}() {{ '{path}' \"$@\"; }}",
                name = name,
                path = path_to_bin
            ),
            StdShell::PowerShell => format!(
                "function {name} {{ & '{path}' @Args }}",
                name = name,
                path = path_to_bin
            ),
        };

        self.with_init_command(alias_command)
    }

    /// Gets path to the specified cargo binary.
    fn cargo_bin(path: impl AsRef<Path>) -> PathBuf {
        let mut path = Self::target_path().join(path);
        path.set_extension(env::consts::EXE_EXTENSION);
        path
    }
}

impl Transcript {
    /// Constructs a transcript from the sequence of given user `input`s.
    ///
    /// The inputs are executed in the shell specified in `options`. A single shell is shared
    /// among all commands.
    ///
    /// # Errors
    ///
    /// - Returns an error if spawning the shell or any operations with it fail (such as reading
    ///   stdout / stderr, or writing commands to stdin).
    #[allow(clippy::missing_panics_doc)] // false positive
    pub fn from_inputs(
        options: &mut ShellOptions,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Self> {
        let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
        let mut shell = options
            .command
            .stdin(Stdio::piped())
            .stdout(pipe_writer.try_clone()?)
            .stderr(pipe_writer)
            .spawn()?;

        let stdout = BufReader::new(pipe_reader);
        let (out_lines_send, out_lines_recv) = mpsc::channel();
        let io_handle = thread::spawn(move || {
            let mut lines = stdout.lines();
            while let Some(Ok(line)) = lines.next() {
                if out_lines_send.send(line).is_err() {
                    break; // the receiver was dropped, we don't care any more
                }
            }
        });

        let stdin = shell.stdin.take().unwrap();
        // ^-- `unwrap()` is safe due to configuration of the shell process.
        let mut stdin = LineWriter::new(stdin);

        // Push initialization commands.
        for cmd in &options.init_commands {
            writeln!(stdin, "{}", cmd)?;
        }
        // Drain all output.
        while out_lines_recv.recv_timeout(options.io_timeout).is_ok() {
            // Intentionally empty.
        }

        let mut transcript = Self::new();
        for input in inputs {
            writeln!(stdin, "{}", input.text)?;

            let mut output = String::new();
            while let Ok(line) = out_lines_recv.recv_timeout(options.io_timeout) {
                if let Some(mapped_line) = (options.line_mapper)(line) {
                    output.push_str(&mapped_line);
                    output.push('\n');
                }
            }
            if output.ends_with('\n') {
                output.truncate(output.len() - 1);
            }
            transcript.interactions.push(Interaction {
                input,
                output: Captured::new(output.into_bytes()),
            });
        }

        drop(stdin); // signals to shell that we're done

        // Drop pipe writers. This is necessary for the pipe reader to receive EOF.
        options.command.stdout(Stdio::null()).stderr(Stdio::null());

        // Give a chance for the shell process to exit. This will reduce kill errors later.
        thread::sleep(options.io_timeout / 4);

        if shell.try_wait()?.is_none() {
            shell.kill().or_else(|err| {
                if Self::is_recoverable_kill_error(&err) {
                    // The shell has already exited. We don't consider this an error.
                    Ok(())
                } else {
                    Err(err)
                }
            })?;
        }

        io_handle.join().ok(); // the I/O thread should not panic
        Ok(transcript)
    }

    #[cfg(not(windows))]
    fn is_recoverable_kill_error(err: &io::Error) -> bool {
        matches!(err.kind(), io::ErrorKind::InvalidInput)
    }

    #[cfg(windows)]
    fn is_recoverable_kill_error(err: &io::Error) -> bool {
        // As per `TerminateProcess` docs (`TerminateProcess` is used by `Child::kill()`),
        // the call will result in ERROR_ACCESS_DENIED if the process has already terminated.
        //
        // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess
        matches!(
            err.kind(),
            io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied
        )
    }

    /// Captures stdout / stderr of the provided `command` and adds it to [`Self::interactions()`].
    ///
    /// The `command` is spawned with closed stdin. This method blocks until the command exits.
    /// The method succeeds regardless of the exit status.
    ///
    /// # Errors
    ///
    /// - Returns an error if spawning the `command` or any operations with it fail (such as reading
    ///   stdout / stderr).
    pub fn capture_output(
        &mut self,
        input: UserInput,
        command: &mut Command,
    ) -> io::Result<&mut Self> {
        let (mut pipe_reader, pipe_writer) = os_pipe::pipe()?;
        let mut child = command
            .stdin(Stdio::null())
            .stdout(pipe_writer.try_clone()?)
            .stderr(pipe_writer)
            .spawn()?;

        // Drop pipe writers. This is necessary for the pipe reader to receive EOF.
        command.stdout(Stdio::null()).stderr(Stdio::null());

        let mut output = vec![];
        pipe_reader.read_to_end(&mut output)?;
        child.wait()?;

        self.interactions.push(Interaction {
            input,
            output: Captured::new(output),
        });
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str;

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
            let output = str::from_utf8(interaction.output().as_ref())?;
            assert_eq!(output.trim(), "hello");
        }

        let interaction = &transcript.interactions()[1];
        assert_eq!(interaction.input().text, "echo foo && echo bar >&2");
        let output = str::from_utf8(&interaction.output().as_ref())?;
        assert_eq!(
            output.split_whitespace().collect::<Vec<_>>(),
            ["foo", "bar"]
        );
        Ok(())
    }
}
