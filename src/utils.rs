//! Misc utils.

use handlebars::Output;

use std::{
    io::{self, Write},
    str,
};

/// [`Output`] implementation that writes to an owned [`String`].
#[derive(Debug, Default)]
pub(crate) struct StringOutput(String);

impl StringOutput {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Output for StringOutput {
    fn write(&mut self, seg: &str) -> io::Result<()> {
        self.0.push_str(seg);
        Ok(())
    }
}

/// Adapter for `dyn Output` that implements `io::Write`.
pub(crate) struct WriteAdapter<'a> {
    inner: &'a mut dyn Output,
}

impl<'a> WriteAdapter<'a> {
    pub fn new(output: &'a mut dyn Output) -> Self {
        Self { inner: output }
    }
}

impl io::Write for WriteAdapter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let segment =
            str::from_utf8(buf).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        self.inner.write(segment)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct IndentingWriter<W> {
    inner: W,
    padding: &'static [u8],
    new_line: bool,
}

impl<W: Write> IndentingWriter<W> {
    pub fn new(writer: W, padding: &'static [u8]) -> Self {
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
                self.inner.write_all(self.padding)?;
            }
            self.inner.write_all(line)?;
        }
        self.new_line = buf.ends_with(b"\n");
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indenting_writer_basics() -> io::Result<()> {
        let mut buffer = vec![];
        let mut writer = IndentingWriter::new(&mut buffer, b"  ");
        write!(writer, "Hello, ")?;
        writeln!(writer, "world!")?;
        writeln!(writer, "many\n  lines!")?;

        assert_eq!(buffer, b"  Hello, world!\n  many\n    lines!\n" as &[u8]);
        Ok(())
    }
}
