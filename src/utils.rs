//! Misc utils.

use handlebars::Output;

use std::{io, str};

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
