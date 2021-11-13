//! Misc utils.

use std::{borrow::Cow, fmt::Write as WriteStr, io, str};

#[cfg(any(feature = "svg", feature = "test"))]
pub use self::rgb_color::{RgbColor, RgbColorParseError};

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

#[cfg(any(feature = "svg", feature = "test"))]
mod rgb_color {
    use std::{error::Error as StdError, fmt, num::ParseIntError, str::FromStr};

    /// RGB color with 8-bit channels.
    ///
    /// A color [can be parsed](FromStr) from a hex string like `#fed` or `#de382b`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RgbColor(pub u8, pub u8, pub u8);

    impl fmt::LowerHex for RgbColor {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
        }
    }

    /// Errors that can occur when [parsing](FromStr) an [`RgbColor`] from a string.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum RgbColorParseError {
        /// The color does not have `#` prefix.
        NoHashPrefix,
        /// The color has incorrect string length (not 1 or 2 chars per color channel).
        IncorrectLen(usize),
        /// Error parsing color channel value.
        IncorrectDigit(ParseIntError),
    }

    impl fmt::Display for RgbColorParseError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::NoHashPrefix => formatter.write_str("Missing '#' prefix"),
                Self::IncorrectLen(len) => write!(
                    formatter,
                    "Unexpected color length {}, expected 4 or 7",
                    len
                ),
                Self::IncorrectDigit(err) => write!(formatter, "Error parsing hex digit: {}", err),
            }
        }
    }

    impl StdError for RgbColorParseError {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            match self {
                Self::IncorrectDigit(err) => Some(err),
                _ => None,
            }
        }
    }

    impl FromStr for RgbColor {
        type Err = RgbColorParseError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if s.is_empty() || s.as_bytes()[0] != b'#' {
                Err(RgbColorParseError::NoHashPrefix)
            } else if s.len() == 4 {
                let r =
                    u8::from_str_radix(&s[1..2], 16).map_err(RgbColorParseError::IncorrectDigit)?;
                let g =
                    u8::from_str_radix(&s[2..3], 16).map_err(RgbColorParseError::IncorrectDigit)?;
                let b =
                    u8::from_str_radix(&s[3..], 16).map_err(RgbColorParseError::IncorrectDigit)?;
                Ok(Self(r * 17, g * 17, b * 17))
            } else if s.len() == 7 {
                let r =
                    u8::from_str_radix(&s[1..3], 16).map_err(RgbColorParseError::IncorrectDigit)?;
                let g =
                    u8::from_str_radix(&s[3..5], 16).map_err(RgbColorParseError::IncorrectDigit)?;
                let b =
                    u8::from_str_radix(&s[5..], 16).map_err(RgbColorParseError::IncorrectDigit)?;
                Ok(Self(r, g, b))
            } else {
                Err(RgbColorParseError::IncorrectLen(s.len()))
            }
        }
    }
}
