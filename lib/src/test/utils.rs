use std::{
    fmt,
    io::{self, Write},
    str,
};

use anstream::{ColorChoice, StripStream};

#[cfg(test)]
use self::tests::print_to_buffer;

// Patch `print!` / `println!` macros for testing similarly to how they are patched in `std`.
#[cfg(test)]
macro_rules! print {
    ($($arg:tt)*) => (print_to_buffer(std::format_args!($($arg)*)));
}
#[cfg(test)]
macro_rules! println {
    ($($arg:tt)*) => {
        print_to_buffer(std::format_args!($($arg)*));
        print_to_buffer(std::format_args!("\n"));
    }
}

/// Writer that adds `padding` to each printed line.
#[derive(Debug)]
pub(super) struct IndentingWriter<W> {
    inner: W,
    padding: &'static str,
    new_line: bool,
}

impl<W: Write> IndentingWriter<W> {
    pub(super) fn new(writer: W, padding: &'static str) -> Self {
        Self {
            inner: writer,
            padding,
            new_line: true,
        }
    }
}

impl<W: Write> Write for IndentingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for (i, line) in buf.split(|&c| c == b'\n').enumerate() {
            if i > 0 {
                self.inner.write_all(b"\n")?;
            }
            if !line.is_empty() && (i > 0 || self.new_line) {
                self.inner.write_all(self.padding.as_bytes())?;
            }
            self.inner.write_all(line)?;
        }
        self.new_line = buf.ends_with(b"\n");
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `Write`r that uses `print!` / `println!` for output.
///
/// # Why is this needed?
///
/// This writer is used to output text within `TestConfig`. The primary use case of
/// `TestConfig` is to be used within tests, and there the output is captured by default,
/// which is implemented by effectively overriding the `std::print*` family of macros
/// (see `std::io::_print()` for details). Using `termcolor::StandardStream` or another `Write`r
/// connected to stdout will lead to `TestConfig` output not being captured,
/// resulting in weird / incomprehensible test output.
///
/// This issue is solved by using a writer that uses `std::print*` macros internally,
/// instead of (implicitly) binding to `std::io::stdout()`.
#[derive(Debug, Default)]
pub(crate) struct PrintlnWriter {
    line_buffer: Vec<u8>,
}

impl Write for PrintlnWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for (i, line) in buf.split(|&c| c == b'\n').enumerate() {
            if i > 0 {
                // Output previously saved line and clear the line buffer.
                let str = str::from_utf8(&self.line_buffer)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
                println!("{str}");
                self.line_buffer.clear();
            }
            self.line_buffer.extend_from_slice(line);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let str = str::from_utf8(&self.line_buffer)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        print!("{str}");
        self.line_buffer.clear();
        Ok(())
    }
}

pub(crate) enum ChoiceWriter<W> {
    Passthrough(W),
    Strip(StripStream<Box<dyn Write>>),
}

impl<W> fmt::Debug for ChoiceWriter<W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passthrough(_) => formatter.debug_tuple("Passthrough").finish_non_exhaustive(),
            Self::Strip(_) => formatter.debug_tuple("Strip").finish_non_exhaustive(),
        }
    }
}

impl<W: Write + 'static> ChoiceWriter<W> {
    pub(crate) fn new(inner: W, choice: ColorChoice) -> Self {
        match choice {
            ColorChoice::Always | ColorChoice::AlwaysAnsi => Self::Passthrough(inner),
            ColorChoice::Never => Self::Strip(StripStream::new(Box::new(inner))),
            ColorChoice::Auto => unreachable!("must be resolved"),
        }
    }
}

impl<W: Write + 'static> Write for ChoiceWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Passthrough(inner) => inner.write(buf),
            Self::Strip(inner) => inner.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Passthrough(inner) => inner.flush(),
            Self::Strip(inner) => inner.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fmt, mem};

    use super::*;

    thread_local! {
        static OUTPUT_CAPTURE: RefCell<Vec<u8>> = RefCell::default();
    }

    pub(super) fn print_to_buffer(args: fmt::Arguments<'_>) {
        OUTPUT_CAPTURE.with(|capture| {
            let mut lock = capture.borrow_mut();
            lock.write_fmt(args).ok();
        });
    }

    #[test]
    fn indenting_writer_basics() -> io::Result<()> {
        let mut buffer = vec![];
        let mut writer = IndentingWriter::new(&mut buffer, "  ");
        write!(writer, "Hello, ")?;
        writeln!(writer, "world!")?;
        writeln!(writer, "many\n  lines!")?;

        assert_eq!(buffer, b"  Hello, world!\n  many\n    lines!\n");
        Ok(())
    }

    #[test]
    fn println_writer_basics() -> io::Result<()> {
        let mut writer = PrintlnWriter::default();
        write!(writer, "Hello, ")?;
        writeln!(writer, "world!")?;
        writeln!(writer, "many\n  lines!")?;

        let captured = OUTPUT_CAPTURE.with(|capture| {
            let mut lock = capture.borrow_mut();
            mem::take(&mut *lock)
        });

        assert_eq!(captured, b"Hello, world!\nmany\n  lines!\n");
        Ok(())
    }
}
