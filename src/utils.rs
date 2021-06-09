//! Misc utils.

use std::{borrow::Cow, fmt::Write as WriteStr, io, str};

/// Adapter for `dyn fmt::Write` that implements `io::Write`.
pub(crate) struct WriteAdapter<'a> {
    inner: &'a mut dyn WriteStr,
}

impl<'a> WriteAdapter<'a> {
    pub fn new(output: &'a mut dyn WriteStr) -> Self {
        Self { inner: output }
    }
}

impl io::Write for WriteAdapter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let segment =
            str::from_utf8(buf).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        self.inner
            .write_str(segment)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn normalize_newlines(s: &str) -> Cow<'_, str> {
    if s.contains("\r\n") {
        Cow::Owned(s.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

#[cfg(not(windows))]
pub(crate) fn is_recoverable_kill_error(err: &io::Error) -> bool {
    matches!(err.kind(), io::ErrorKind::InvalidInput)
}

// As per `TerminateProcess` docs (`TerminateProcess` is used by `Child::kill()`),
// the call will result in ERROR_ACCESS_DENIED if the process has already terminated.
//
// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess
#[cfg(windows)]
pub(crate) fn is_recoverable_kill_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied
    )
}
