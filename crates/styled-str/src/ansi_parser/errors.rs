use core::{fmt, num::ParseIntError, str::Utf8Error};

use crate::alloc::String;

/// Errors that can occur when processing terminal output.
#[derive(Debug)]
#[non_exhaustive]
pub enum AnsiError {
    /// Unfinished escape sequence.
    UnfinishedSequence,
    /// Unrecognized escape sequence (not a CSI or OSC one). The enclosed byte
    /// is the first byte of the sequence (excluding `0x1b`).
    UnrecognizedSequence(u8),
    /// Invalid final byte for an SGR escape sequence.
    InvalidSgrFinalByte(u8),
    /// Unfinished color spec.
    UnfinishedColor,
    /// Invalid type of a color spec.
    InvalidColorType(String),
    /// Invalid ANSI color index.
    InvalidColorIndex(ParseIntError),
    /// UTF-8 decoding error.
    Utf8(Utf8Error),
}

impl fmt::Display for AnsiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnfinishedSequence => formatter.write_str("Unfinished ANSI escape sequence"),
            Self::UnrecognizedSequence(byte) => {
                write!(
                    formatter,
                    "Unrecognized escape sequence (first byte is {byte})"
                )
            }
            Self::InvalidSgrFinalByte(byte) => {
                write!(
                    formatter,
                    "Invalid final byte for an SGR escape sequence: {byte}"
                )
            }
            Self::UnfinishedColor => formatter.write_str("Unfinished color spec"),
            Self::InvalidColorType(ty) => {
                write!(formatter, "Invalid type of a color spec: {ty}")
            }
            Self::InvalidColorIndex(err) => {
                write!(formatter, "Failed parsing color index: {err}")
            }
            Self::Utf8(err) => write!(formatter, "UTF-8 decoding error: {err}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AnsiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidColorIndex(err) => Some(err),
            Self::Utf8(err) => Some(err),
            _ => None,
        }
    }
}

impl From<Utf8Error> for AnsiError {
    fn from(err: Utf8Error) -> Self {
        Self::Utf8(err)
    }
}
