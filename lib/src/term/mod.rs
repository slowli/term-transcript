use std::borrow::Cow;

use crate::{TermError, utils::normalize_newlines};

mod parser;
#[cfg(test)]
mod tests;

pub(crate) use self::parser::TermOutputParser;

/// Marker trait for supported types of terminal output.
pub trait TermOutput: Clone + Send + Sync + 'static {}

/// Output captured from the terminal.
#[derive(Debug, Clone)]
pub struct Captured(String);

impl AsRef<str> for Captured {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for Captured {
    fn from(raw: String) -> Self {
        // Normalize newlines to `\n`.
        Self(match normalize_newlines(&raw) {
            Cow::Owned(normalized) => normalized,
            Cow::Borrowed(_) => raw,
        })
    }
}

impl Captured {
    fn write_as_plaintext(&self, output: &mut String) -> Result<(), TermError> {
        TermOutputParser::new(output).parse(self.0.as_bytes())
    }

    /// Converts this terminal output to plaintext.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn to_plaintext(&self) -> Result<String, TermError> {
        let mut output = String::with_capacity(self.0.len());
        self.write_as_plaintext(&mut output)?;
        Ok(output)
    }
}

impl TermOutput for Captured {}
