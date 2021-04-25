use handlebars::Output;
use termcolor::NoColor;

use std::borrow::Cow;

use crate::{
    html::HtmlWriter,
    test::MatchKind,
    utils::{normalize_newlines, StringOutput, WriteAdapter},
    TermError,
};

mod parser;
use self::parser::TermOutputParser;

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

impl Captured {
    pub(crate) fn new(term_output: String) -> Self {
        // Normalize newlines to `\n`.
        Self(match normalize_newlines(&term_output) {
            Cow::Owned(normalized) => normalized,
            Cow::Borrowed(_) => term_output,
        })
    }

    /// Writes this output in the HTML format to the provided `writer`.
    ///
    /// FIXME: more about HTML formatting
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn write_as_html(&self, output: &mut dyn Output) -> Result<(), TermError> {
        let mut html_writer = HtmlWriter::new(output);
        TermOutputParser::new(&mut html_writer).parse(self.0.as_bytes())
    }

    /// Convenience method for converting this terminal output to an HTML string
    /// using [`Self::write_as_html()`].
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn to_html(&self) -> Result<String, TermError> {
        let mut output = StringOutput::default();
        self.write_as_html(&mut output)?;
        Ok(output.into_inner())
    }

    /// Writes this output in the plaintext format to the provided `writer`.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn write_as_plaintext(&self, output: &mut dyn Output) -> Result<(), TermError> {
        let mut plaintext_writer = NoColor::new(WriteAdapter::new(output));
        TermOutputParser::new(&mut plaintext_writer).parse(self.0.as_bytes())
    }

    /// Convenience method for converting this terminal output to plaintext
    /// using [`Self::write_as_plaintext()`].
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    pub fn to_plaintext(&self) -> Result<String, TermError> {
        let mut output = StringOutput::default();
        self.write_as_plaintext(&mut output)?;
        Ok(output.into_inner())
    }
}

impl TermOutput for Captured {}

/// Parsed terminal output.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub(crate) plaintext: String,
    pub(crate) html: String,
}

impl Parsed {
    /// Gets the parsed plaintext.
    pub fn plaintext(&self) -> &str {
        &self.plaintext
    }

    /// Gets the parsed HTML.
    pub fn html(&self) -> &str {
        &self.html
    }

    /// Asserts that this parsed output matches `captured` terminal output.
    ///
    /// # Panics
    ///
    /// - Panics if `captured` output cannot be converted to plaintext / HTML.
    /// - Panics if the assertion fails. If the `pretty_assertions` feature is enabled,
    ///   the output will be formatted using the [corresponding crate].
    ///
    /// [corresponding crate]: https://docs.rs/pretty_assertions/
    pub fn assert_matches(&self, captured: &Captured, match_kind: MatchKind) {
        #[cfg(feature = "pretty_assertions")]
        use pretty_assertions::assert_eq;

        match match_kind {
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use termcolor::{Ansi, Color, ColorSpec, WriteColor};

    fn prepare_term_output() -> anyhow::Result<String> {
        let mut writer = Ansi::new(vec![]);
        writer.set_color(
            ColorSpec::new()
                .set_fg(Some(Color::Cyan))
                .set_underline(true),
        )?;
        write!(writer, "Hello")?;
        writer.reset()?;
        write!(writer, ", ")?;
        writer.set_color(
            ColorSpec::new()
                .set_fg(Some(Color::White))
                .set_bg(Some(Color::Green))
                .set_intense(true),
        )?;
        write!(writer, "world")?;
        writer.reset()?;
        write!(writer, "!")?;

        String::from_utf8(writer.into_inner()).map_err(From::from)
    }

    const EXPECTED_HTML: &str = "<span class=\"underline fg-cyan\">Hello</span>, \
         <span class=\"fg-i-white bg-i-green\">world</span>!";

    #[test]
    fn converting_captured_output_to_text() -> anyhow::Result<()> {
        let output = Captured(prepare_term_output()?);
        assert_eq!(output.to_plaintext()?, "Hello, world!");
        Ok(())
    }

    #[test]
    fn converting_captured_output_to_html() -> anyhow::Result<()> {
        let output = Captured(prepare_term_output()?);
        assert_eq!(output.to_html()?, EXPECTED_HTML);
        Ok(())
    }

    #[test]
    fn matching_text() {
        let captured = Captured("Hello, world!".to_owned());
        let parsed = Parsed {
            plaintext: "Hello, world!".to_owned(),
            html: EXPECTED_HTML.to_owned(),
        };

        parsed.assert_matches(&captured, MatchKind::TextOnly);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn matching_imprecise_text() {
        let captured = Captured("Hello, world!".to_owned());
        let parsed = Parsed {
            plaintext: "Hello, world!".to_owned(),
            html: EXPECTED_HTML.to_owned(),
        };

        parsed.assert_matches(&captured, MatchKind::Precise);
    }

    #[test]
    fn matching_text_and_colors() {
        let captured = Captured(prepare_term_output().unwrap());
        let parsed = Parsed {
            plaintext: "Hello, world!".to_owned(),
            html: EXPECTED_HTML.to_owned(),
        };

        parsed.assert_matches(&captured, MatchKind::TextOnly);
        parsed.assert_matches(&captured, MatchKind::Precise);
    }
}
