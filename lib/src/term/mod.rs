use std::{borrow::Cow, fmt::Write as WriteStr};

use termcolor::NoColor;

#[cfg(feature = "svg")]
use crate::write::SvgLine;
use crate::{
    utils::{normalize_newlines, WriteAdapter},
    write::{GenericWriter, HtmlLine},
    TermError,
};

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
    pub(crate) fn write_as_html(
        &self,
        wrap_width: Option<usize>,
    ) -> Result<Vec<HtmlLine>, TermError> {
        let mut html_writer = GenericWriter::<HtmlLine>::new(wrap_width);
        TermOutputParser::new(&mut html_writer).parse(self.0.as_bytes())?;
        Ok(html_writer.into_lines())
    }

    #[cfg(feature = "svg")]
    pub(crate) fn write_as_svg(
        &self,
        wrap_width: Option<usize>,
    ) -> Result<Vec<SvgLine>, TermError> {
        let mut svg_writer = GenericWriter::<SvgLine>::new(wrap_width);
        TermOutputParser::new(&mut svg_writer).parse(self.0.as_bytes())?;
        Ok(svg_writer.into_lines())
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
        let lines = self.write_as_html(None)?;
        let output = lines.into_iter().fold(String::new(), |mut acc, line| {
            acc.push_str(&line.html);
            // FIXME: take line breaks into account
            acc
        });
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
