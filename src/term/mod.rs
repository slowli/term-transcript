use termcolor::NoColor;

use std::{borrow::Cow, fmt::Write as WriteStr};

use crate::{
    html::HtmlWriter,
    utils::{normalize_newlines, WriteAdapter},
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

    pub(crate) fn write_as_html(
        &self,
        output: &mut dyn WriteStr,
        wrap_width: Option<usize>,
    ) -> Result<(), TermError> {
        let mut html_writer = HtmlWriter::new(output, wrap_width);
        TermOutputParser::new(&mut html_writer).parse(self.0.as_bytes())
    }

    /// Converts this terminal output to an HTML string.
    ///
    /// The conversion applies styles by wrapping colored / styled text into `span`s with
    /// the following `class`es:
    ///
    /// - `bold`, `italic`, `dimmed`, `underline` are self-explanatory
    /// - `fg0`, `fg1`, ..., `fg15` are used to indicate indexed 4-bit ANSI color of the text.
    ///   Indexes 0..=7 correspond to the ordinary color variations, and 8..=15
    ///   to the intense ones.
    /// - `bg0`, `bg1`, ..., `bg15` work similarly, but for the background color instead of
    ///   text color.
    ///
    /// Indexed ANSI colors with indexes >15 and ANSI RGB colors are rendered using the `style`
    /// attribute.
    ///
    /// The output string retains whitespace of the input. Hence, it needs to be wrapped
    /// into a `pre` element or an element with the [`white-space`] CSS property set to `pre`
    /// in order to be displayed properly.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an issue processing output.
    ///
    /// [`white-space`]: https://developer.mozilla.org/en-US/docs/Web/CSS/white-space
    pub fn to_html(&self) -> Result<String, TermError> {
        let mut output = String::with_capacity(self.0.len());
        self.write_as_html(&mut output, None)?;
        Ok(output)
    }

    fn write_as_plaintext(&self, output: &mut dyn WriteStr) -> Result<(), TermError> {
        let mut plaintext_writer = NoColor::new(WriteAdapter::new(output));
        TermOutputParser::new(&mut plaintext_writer).parse(self.0.as_bytes())
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

    const EXPECTED_HTML: &str = "<span class=\"underline fg6\">Hello</span>, \
         <span class=\"fg15 bg10\">world</span>!";

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
}
