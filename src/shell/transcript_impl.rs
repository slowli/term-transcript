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
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(writer), err)
    )]
    fn write_line(writer: &mut impl io::Write, line: &str) -> io::Result<()> {
        writeln!(writer, "{line}")
    }

    // Lines need to end with `\r\n` to be properly processed, at least when writing to a PTY.
    #[cfg(windows)]
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(writer), err)
    )]
    fn write_line(writer: &mut impl io::Write, line: &str) -> io::Result<()> {
        writeln!(writer, "{line}\r")
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(lines_recv), err)
    )]
    #[cfg_attr(not(feature = "tracing"), allow(unused_variables))]
    // ^ The received `line` is used only for debug purposes
    fn read_echo(
        input_line: &str,
        lines_recv: &mpsc::Receiver<Vec<u8>>,
        io_timeout: Duration,
    ) -> io::Result<()> {
        if let Ok(line) = lines_recv.recv_timeout(io_timeout) {
            #[cfg(feature = "tracing")]
            tracing::debug!(line_utf8 = std::str::from_utf8(&line).ok(), "received line");
            Ok(())
        } else {
            let err =
                format!("could not read all input `{input_line}` back from an echoing terminal");
            Err(io::Error::new(io::ErrorKind::BrokenPipe, err))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(lines_recv, line_decoder), ret, err)
    )]
    fn read_output(
        lines_recv: &mpsc::Receiver<Vec<u8>>,
        io_timeout: Duration,
        line_decoder: &mut impl FnMut(Vec<u8>) -> io::Result<String>,
    ) -> io::Result<String> {
        let mut output = String::new();
        while let Ok(mut line) = lines_recv.recv_timeout(io_timeout) {
            if line.last() == Some(&b'\r') {
                // Normalize `\r\n` line ending to `\n`.
                line.pop();
            }
            #[cfg(feature = "tracing")]
            tracing::debug!(line_utf8 = std::str::from_utf8(&line).ok(), "received line");

            let mapped_line = line_decoder(line)?;
            #[cfg(feature = "tracing")]
            tracing::debug!(?mapped_line, "mapped received line");
            output.push_str(&mapped_line);
            output.push('\n');
        }

        if output.ends_with('\n') {
            output.truncate(output.len() - 1);
        }
        Ok(output)
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
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            skip_all,
            err,
            fields(
                options.io_timeout = ?options.io_timeout,
                options.init_timeout = ?options.init_timeout,
                options.path_additions = ?options.path_additions,
                options.init_commands = ?options.init_commands
            )
        )
    )]
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
            #[cfg(feature = "tracing")]
            let _entered = tracing::debug_span!("reader_thread").entered();

            let mut lines = stdout.split(b'\n');
            while let Some(Ok(line)) = lines.next() {
                #[cfg(feature = "tracing")]
                tracing::debug!(line_utf8 = std::str::from_utf8(&line).ok(), "received line");

                if out_lines_send.send(line).is_err() {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("receiver dropped, breaking reader loop");
                    break;
                }
            }
        });

        let mut stdin = LineWriter::new(writer);
        Self::push_init_commands(options, &out_lines_recv, &mut shell, &mut stdin)?;

        let mut transcript = Self::new();
        for input in inputs {
            let interaction =
                Self::record_interaction(options, input, &out_lines_recv, &mut shell, &mut stdin)?;
            transcript.interactions.push(interaction);
        }

        drop(stdin); // signals to shell that we're done

        // Give a chance for the shell process to exit. This will reduce kill errors later.
        thread::sleep(options.io_timeout / 4);

        shell.terminate()?;
        io_handle.join().ok(); // the I/O thread should not panic, so we ignore errors here
        Ok(transcript)
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            level = "debug",
            skip_all,
            err,
            fields(options.init_commands = ?options.init_commands)
        )
    )]
    fn push_init_commands<Cmd: SpawnShell>(
        options: &ShellOptions<Cmd>,
        lines_recv: &mpsc::Receiver<Vec<u8>>,
        shell: &mut Cmd::ShellProcess,
        stdin: &mut impl io::Write,
    ) -> io::Result<()> {
        let mut timeouts = Timeouts::new(options);

        // Push initialization commands.
        if shell.is_echoing() {
            for cmd in &options.init_commands {
                Self::write_line(stdin, cmd)?;
                Self::read_echo(cmd, lines_recv, timeouts.next())?;

                // Drain all other output as well.
                while lines_recv.recv_timeout(timeouts.next()).is_ok() {
                    // Intentionally empty.
                }
            }
        } else {
            // Since we don't care about getting all echoes back, we can push all lines at once and
            // drain the output afterwards.
            for cmd in &options.init_commands {
                Self::write_line(stdin, cmd)?;
            }
        }

        // Drain all output left after commands and let the shell get fully initialized.
        while lines_recv.recv_timeout(timeouts.next()).is_ok() {
            // Intentionally empty.
        }
        // At this point, at least one item was requested from `timeout_iter`, so the further code
        // may safely use `options.io_timeout`.
        Ok(())
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(options, lines_recv, shell, stdin), ret, err)
    )]
    fn record_interaction<Cmd: SpawnShell>(
        options: &mut ShellOptions<Cmd>,
        input: UserInput,
        lines_recv: &mpsc::Receiver<Vec<u8>>,
        shell: &mut Cmd::ShellProcess,
        stdin: &mut impl io::Write,
    ) -> io::Result<Interaction> {
        // Check if the shell is still alive. It seems that older Rust versions allow
        // to write to `stdin` even after the shell exits.
        shell.check_is_alive()?;

        let input_lines = input.text.split('\n');
        for input_line in input_lines {
            Self::write_line(stdin, input_line)?;
            if shell.is_echoing() {
                Self::read_echo(input_line, lines_recv, options.io_timeout)?;
            }
        }

        let output = Self::read_output(lines_recv, options.io_timeout, &mut options.line_decoder)?;

        let exit_status = if let Some(status_check) = &options.status_check {
            let command = status_check.command();
            Self::write_line(stdin, command)?;
            if shell.is_echoing() {
                Self::read_echo(command, lines_recv, options.io_timeout)?;
            }
            let response =
                Self::read_output(lines_recv, options.io_timeout, &mut options.line_decoder)?;
            status_check.check(&Captured::new(response))
        } else {
            None
        };

        let mut interaction = Interaction::new(input, output);
        interaction.exit_status = exit_status;
        Ok(interaction)
    }

    /// Captures stdout / stderr of the provided `command` and adds it to [`Self::interactions()`].
    ///
    /// The `command` is spawned with the closed stdin. This method blocks until the command exits.
    /// The method succeeds regardless of the exit status of the `command`.
    ///
    /// # Errors
    ///
    /// - Returns an error if spawning the `command` or any operations with it fail (such as reading
    ///   stdout / stderr).
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(skip(self, input), err, fields(input.text = %input.text))
    )]
    pub fn capture_output(
        &mut self,
        input: UserInput,
        command: &mut Command,
    ) -> io::Result<&mut Self> {
        let (mut pipe_reader, pipe_writer) = os_pipe::pipe()?;
        #[cfg(feature = "tracing")]
        tracing::debug!("created OS pipe");
        let mut child = command
            .stdin(Stdio::null())
            .stdout(pipe_writer.try_clone()?)
            .stderr(pipe_writer)
            .spawn()?;
        #[cfg(feature = "tracing")]
        tracing::debug!("created child");

        // Drop pipe writers. This is necessary for the pipe reader to receive EOF.
        command.stdout(Stdio::null()).stderr(Stdio::null());

        let mut output = vec![];
        pipe_reader.read_to_end(&mut output)?;
        child.wait()?;

        let output = String::from_utf8(output)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.utf8_error()))?;
        #[cfg(feature = "tracing")]
        tracing::debug!(?output, "read command output");

        self.interactions.push(Interaction::new(input, output));
        Ok(self)
    }
}
