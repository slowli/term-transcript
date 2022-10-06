//! Shell-related `Transcript` methods.

use std::{
    io::{self, BufRead, BufReader, LineWriter, Read},
    iter,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use super::ShellOptions;
use crate::{
    traits::{ShellProcess, SpawnShell, SpawnedShell},
    Captured, Interaction, Transcript, UserInput,
};

#[derive(Debug)]
struct Timeouts {
    inner: iter::Chain<iter::Once<Duration>, iter::Repeat<Duration>>,
}

impl Timeouts {
    fn new<Cmd: SpawnShell>(options: &ShellOptions<Cmd>) -> Self {
        Self {
            inner: iter::once(options.init_timeout + options.io_timeout)
                .chain(iter::repeat(options.io_timeout)),
        }
    }

    fn next(&mut self) -> Duration {
        self.inner.next().unwrap() // safe by construction; the iterator is indefinite
    }
}

impl Transcript {
    #[cfg(not(windows))]
    fn write_line(writer: &mut impl io::Write, line: &str) -> io::Result<()> {
        writeln!(writer, "{line}")
    }

    // Lines need to end with `\r\n` to be properly processed, at least when writing to a PTY.
    #[cfg(windows)]
    fn write_line(writer: &mut impl io::Write, line: &str) -> io::Result<()> {
        writeln!(writer, "{line}\r")
    }

    fn read_echo(
        input_line: &str,
        lines_recv: &mpsc::Receiver<Vec<u8>>,
        io_timeout: Duration,
    ) -> io::Result<()> {
        if lines_recv.recv_timeout(io_timeout).is_ok() {
            Ok(())
        } else {
            let err =
                format!("could not read all input `{input_line}` back from an echoing terminal");
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
    ///   stdout / stderr, or writing commands to stdin), or if the shell exits before all commands
    ///   are executed.
    #[allow(clippy::missing_panics_doc)] // false positive
    pub fn from_inputs<Cmd: SpawnShell>(
        options: &mut ShellOptions<Cmd>,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Self> {
        let SpawnedShell {
            mut shell,
            reader,
            writer,
        } = options.spawn_shell()?;

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
        let mut timeouts = Timeouts::new(options);

        // Push initialization commands.
        if shell.is_echoing() {
            for cmd in &options.init_commands {
                Self::write_line(&mut stdin, cmd)?;
                Self::read_echo(cmd, &out_lines_recv, timeouts.next())?;

                // Drain all other output as well.
                while out_lines_recv.recv_timeout(timeouts.next()).is_ok() {
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
        while out_lines_recv.recv_timeout(timeouts.next()).is_ok() {
            // Intentionally empty.
        }
        // At this point, at least one item was requested from `timeout_iter`, so the further code
        // may safely use `options.io_timeout`.

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
    /// The method succeeds regardless of the exit status of the `command`.
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
