//! Shell-related types.

use std::{
    env,
    ffi::OsStr,
    fmt,
    io::{self, BufRead, BufReader, LineWriter, Read},
    path::{Path, PathBuf},
    process::{ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::{
    traits::{ChildShell, ConfigureCommand, ShellProcess, SpawnShell, SpawnedShell},
    Captured, Interaction, Transcript, UserInput,
};

/// Options for executing commands in the shell. Used in [`Transcript::from_inputs()`]
/// and in [`TestConfig`].
///
/// The type param maps to *extensions* available for the shell. For example, [`StdShell`]
/// extension allows to specify custom aliases for the executed commands.
///
/// [`TestConfig`]: crate::test::TestConfig
pub struct ShellOptions<Cmd = Command> {
    command: Cmd,
    io_timeout: Duration,
    init_commands: Vec<String>,
    line_mapper: Box<dyn FnMut(String) -> Option<String>>,
}

impl<Cmd: fmt::Debug> fmt::Debug for ShellOptions<Cmd> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShellOptions")
            .field("command", &self.command)
            .field("io_timeout", &self.io_timeout)
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
            io_timeout: Duration::from_secs(1),
            init_commands: vec![],
            line_mapper: Box::new(Some),
        }
    }

    /// Changes the current directory of the command.
    pub fn with_current_dir(mut self, current_dir: impl AsRef<Path>) -> Self {
        self.command.current_dir(current_dir.as_ref());
        self
    }

    /// Sets the I/O timeout for shell commands. This determines how long methods like
    /// [`Transcript::from_inputs()`] wait
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

    /// Sets the `value` of an environment variable with the specified `name`.
    pub fn with_env(mut self, name: impl AsRef<str>, value: impl AsRef<OsStr>) -> Self {
        self.command.env(name.as_ref(), value.as_ref());
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
}

#[derive(Debug, Clone, Copy)]
enum StdShellType {
    /// `sh` shell.
    Sh,
    /// `bash` shell.
    Bash,
    /// PowerShell.
    PowerShell,
}

/// Shell interpreter that brings additional functionality for [`ShellOptions`].
#[derive(Debug)]
pub struct StdShell {
    shell_type: StdShellType,
    command: Command,
}

impl ConfigureCommand for StdShell {
    fn current_dir(&mut self, dir: &Path) {
        self.command.current_dir(dir);
    }

    fn env(&mut self, name: &str, value: &OsStr) {
        self.command.env(name, value);
    }
}

impl ShellOptions<StdShell> {
    /// Creates options for an `sh` shell.
    pub fn sh() -> Self {
        Self::new(StdShell {
            shell_type: StdShellType::Sh,
            command: Command::new("sh"),
        })
    }

    /// Creates options for a Bash shell.
    pub fn bash() -> Self {
        Self::new(StdShell {
            shell_type: StdShellType::Bash,
            command: Command::new("bash"),
        })
    }

    /// Creates options for PowerShell.
    #[allow(clippy::doc_markdown)] // false positive
    pub fn powershell() -> Self {
        let mut command = Command::new("powershell");
        command.arg("-NoLogo").arg("-NoExit");

        let command = StdShell {
            shell_type: StdShellType::PowerShell,
            command,
        };
        Self::new(command).with_init_command("function prompt { }")
    }

    /// Creates an alias for the binary at `path_to_bin`, which should be an absolute path.
    /// This allows to call the binary using this alias without complex preparations (such as
    /// installing it globally via `cargo install`), and is more flexible than
    /// [`Self::with_cargo_path()`].
    ///
    /// In integration tests, you may use [`env!("CARGO_BIN_EXE_<name>")`] to get a path
    /// to binary targets.
    ///
    /// # Limitations
    ///
    /// - For Bash and PowerShell, `name` must be a valid name of a function. For `sh`,
    ///   `name` must be a valid name for the `alias` command. The `name` validity
    ///   is **not** checked.
    ///
    /// [`env!("CARGO_BIN_EXE_<name>")`]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    #[allow(clippy::doc_markdown)] // false positive
    pub fn with_alias(self, name: &str, path_to_bin: &str) -> Self {
        let alias_command = match self.command.shell_type {
            StdShellType::Sh => {
                format!("alias {name}=\"'{path}'\"", name = name, path = path_to_bin)
            }
            StdShellType::Bash => format!(
                "{name}() {{ '{path}' \"$@\"; }}",
                name = name,
                path = path_to_bin
            ),
            StdShellType::PowerShell => format!(
                "function {name} {{ & '{path}' @Args }}",
                name = name,
                path = path_to_bin
            ),
        };

        self.with_init_command(alias_command)
    }
}

impl SpawnShell for StdShell {
    type ShellProcess = ChildShell;
    type Reader = os_pipe::PipeReader;
    type Writer = ChildStdin;

    fn spawn_shell(&mut self) -> io::Result<SpawnedShell<Self>> {
        let SpawnedShell {
            mut shell,
            reader,
            writer,
        } = self.command.spawn_shell()?;

        if matches!(self.shell_type, StdShellType::PowerShell) {
            shell.set_echoing();
        }

        Ok(SpawnedShell {
            shell,
            reader,
            writer,
        })
    }
}

impl Transcript {
    #[cfg(not(windows))]
    fn write_line(writer: &mut impl io::Write, line: &str) -> io::Result<()> {
        writeln!(writer, "{}", line)
    }

    // Lines need to end with `\r\n` to be properly processed, at least when writing to a PTY.
    #[cfg(windows)]
    fn write_line(writer: &mut impl io::Write, line: &str) -> io::Result<()> {
        writeln!(writer, "{}\r", line)
    }

    fn read_echo(
        input_line: &str,
        lines_recv: &mpsc::Receiver<Vec<u8>>,
        io_timeout: Duration,
    ) -> io::Result<()> {
        if lines_recv.recv_timeout(io_timeout).is_ok() {
            Ok(())
        } else {
            let err = format!(
                "could not read all input `{}` back from an echoing terminal",
                input_line
            );
            Err(io::Error::new(io::ErrorKind::BrokenPipe, err))
        }
    }

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
    pub fn from_inputs<Cmd: SpawnShell>(
        options: &mut ShellOptions<Cmd>,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Self> {
        let SpawnedShell {
            mut shell,
            reader,
            writer,
        } = options.command.spawn_shell()?;

        let stdout = BufReader::new(reader);
        let (out_lines_send, out_lines_recv) = mpsc::channel();
        let io_handle = thread::spawn(move || {
            let mut lines = stdout.split(b'\n');
            while let Some(Ok(line)) = lines.next() {
                if out_lines_send.send(line).is_err() {
                    break; // the receiver was dropped, we don't care any more
                }
            }
        });

        let mut stdin = LineWriter::new(writer);

        // Push initialization commands.
        if shell.is_echoing() {
            for cmd in &options.init_commands {
                Self::write_line(&mut stdin, cmd)?;
                Self::read_echo(cmd, &out_lines_recv, options.io_timeout)?;

                // Drain all other output as well.
                while out_lines_recv.recv_timeout(options.io_timeout).is_ok() {
                    // Intentionally empty.
                }
            }
        } else {
            // Since we don't care about getting all echoes back, we can push all lines at once and
            // drain the output afterwards.
            for cmd in &options.init_commands {
                Self::write_line(&mut stdin, cmd)?;
            }
        }

        // Drain all output left after commands and let the shell get fully initialized.
        while out_lines_recv.recv_timeout(options.io_timeout).is_ok() {
            // Intentionally empty.
        }

        let mut transcript = Self::new();
        for input in inputs {
            // Check if the shell is still alive. It seems that older Rust versions allow
            // to write to `stdin` even after the shell exits.
            shell.check_is_alive()?;

            let input_lines = input.text.split('\n');
            for input_line in input_lines {
                Self::write_line(&mut stdin, input_line)?;
                if shell.is_echoing() {
                    Self::read_echo(input_line, &out_lines_recv, options.io_timeout)?;
                }
            }

            let mut output = String::new();
            while let Ok(mut line) = out_lines_recv.recv_timeout(options.io_timeout) {
                if line.last() == Some(&b'\r') {
                    // Normalize `\r\n` line ending to `\n`.
                    line.pop();
                }
                let line = String::from_utf8(line)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.utf8_error()))?;

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
                output: Captured::new(output),
            });
        }

        drop(stdin); // signals to shell that we're done

        // Give a chance for the shell process to exit. This will reduce kill errors later.
        thread::sleep(options.io_timeout / 4);

        shell.terminate()?;
        io_handle.join().ok(); // the I/O thread should not panic, so we ignore errors here
        Ok(transcript)
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

        let output = String::from_utf8(output)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.utf8_error()))?;

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
