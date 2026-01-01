use std::{
    io::{self, Write},
    str,
};

#[cfg(test)]
use self::tests::print_to_buffer;
use crate::style::WriteStyled;

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

impl<W: WriteStyled> IndentingWriter<W> {
    pub(super) fn new(writer: W, padding: &'static str) -> Self {
        Self {
            inner: writer,
            padding,
            new_line: true,
        }
    }
}

impl<W: WriteStyled> Write for IndentingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for (i, line) in buf.split(|&c| c == b'\n').enumerate() {
            if i > 0 {
                self.inner.write_text("\n")?;
            }
            if !line.is_empty() && (i > 0 || self.new_line) {
                self.inner.write_text(self.padding)?;
            }
            let line = str::from_utf8(line)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            self.inner.write_text(line)?;
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
        let mut buffer = String::new();
        let mut writer = IndentingWriter::new(&mut buffer, "  ");
        write!(writer, "Hello, ")?;
        writeln!(writer, "world!")?;
        writeln!(writer, "many\n  lines!")?;

        assert_eq!(buffer, "  Hello, world!\n  many\n    lines!\n");
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
