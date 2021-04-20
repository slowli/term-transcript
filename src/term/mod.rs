use handlebars::Output;
use termcolor::NoColor;

use crate::{
    html::HtmlWriter,
    utils::{StringOutput, WriteAdapter},
    Error,
};

mod parser;
use self::parser::TermOutputParser;

/// Marker trait for supported types of terminal output.
pub trait TermOutput: Clone + Send + Sync + 'static {}

/// Output captured from the terminal.
#[derive(Debug, Clone)]
pub struct Captured(Vec<u8>);

impl AsRef<[u8]> for Captured {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// FIXME: unit tests for conversions.
impl Captured {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Writes this output in the HTML format to the provided `writer`.
    ///
    /// FIXME: more about HTML formatting
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn write_as_html(&self, output: &mut dyn Output) -> Result<(), Error> {
        let mut html_writer = HtmlWriter::new(output);
        TermOutputParser::new(&mut html_writer).parse(&self.0)
    }

    /// Convenience method for converting this terminal output to an HTML string
    /// using [`Self::write_as_html()`].
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn to_html(&self) -> Result<String, Error> {
        let mut output = StringOutput::default();
        self.write_as_html(&mut output)?;
        Ok(output.into_inner())
    }

    /// Writes this output in the plaintext format to the provided `writer`.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn write_as_plaintext(&self, output: &mut dyn Output) -> Result<(), Error> {
        let mut plaintext_writer = NoColor::new(WriteAdapter::new(output));
        TermOutputParser::new(&mut plaintext_writer).parse(&self.0)
    }

    /// Convenience method for converting this terminal output to plaintext
    /// using [`Self::write_as_plaintext()`].
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn to_plaintext(&self) -> Result<String, Error> {
        let mut output = StringOutput::default();
        self.write_as_plaintext(&mut output)?;
        Ok(output.into_inner())
    }
}

impl TermOutput for Captured {}

/// Parsed terminal output.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub(crate) plaintext: String,
    pub(crate) html: String,
}

impl Parsed {
    /// Asserts that this parsed output matches `captured` terminal output.
    ///
    /// # Panics
    ///
    /// - Panics if `captured` output cannot be converted to plaintext / HTML.
    /// - Panics if the assertion fails.
    // FIXME: unit tests!
    pub fn assert_matches(&self, captured: &Captured, match_type: MatchKind) {
        #[cfg(feature = "pretty_assertions")]
        use pretty_assertions::assert_eq;

        match match_type {
            MatchKind::TextOnly => {
                let captured_plaintext = captured
                    .to_plaintext()
                    .expect("Cannot convert captured output to plaintext");
                assert_eq!(self.plaintext, captured_plaintext);
            }
            MatchKind::Precise => {
                let captured_html = captured
                    .to_html()
                    .expect("Cannot convert captured output to plaintext");
                assert_eq!(self.html, captured_html);
            }
        }
    }
}

impl TermOutput for Parsed {}

/// Kind of terminal output matching. Used in [`Parsed::assert_matches()`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MatchKind {
    /// Precise matching: compare output together with colors.
    Precise,
    /// Relaxed matching: compare only output text, but not coloring.
    TextOnly,
}
