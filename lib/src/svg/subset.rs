//! Font embedder / subsetter based on the `font-subset` library.

use std::{collections::BTreeSet, fmt, fs, io, path::PathBuf};

use font_subset::{FontReader, ParseError};

use super::{EmbeddedFont, FontEmbedder};

/// Errors produced by [`FontSubsetter`].
#[derive(Debug)]
#[non_exhaustive]
pub enum SubsettingError {
    /// Error reading font file.
    Read {
        /// Path to the font file.
        path: PathBuf,
        /// I/O error that has occurred.
        err: io::Error,
    },
    /// Error parsing the font file.
    Parse(ParseError),
    /// No font family name entry in the font.
    NoFontFamilyName,
    /// Subsetting is disallowed by the font permissions.
    NoSubsetting,
    /// Embedding is disallowed by the font permissions.
    NoEmbedding,
    /// The font misses glyphs for some chars used in the transcript.
    MissingChars(String),
}

impl From<ParseError> for SubsettingError {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

impl fmt::Display for SubsettingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, err } => {
                write!(
                    formatter,
                    "failed reading font at `{}`: {err}",
                    path.display()
                )
            }
            Self::Parse(err) => write!(formatter, "error parsing font: {err}"),
            Self::NoFontFamilyName => formatter.write_str("no family name in font file"),
            Self::NoSubsetting => {
                formatter.write_str("subsetting is disallowed by font permissions")
            }
            Self::NoEmbedding => formatter.write_str("embedding is disallowed by font permissions"),
            Self::MissingChars(chars) => {
                write!(
                    formatter,
                    "font misses glyphs for chars used in transcript: {chars}"
                )
            }
        }
    }
}

impl std::error::Error for SubsettingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { err, .. } => Some(err),
            Self::Parse(err) => Some(err),
            Self::NoFontFamilyName
            | Self::NoSubsetting
            | Self::NoEmbedding
            | Self::MissingChars(_) => None,
        }
    }
}

/// Font embedder / subsetter based on the `font-subset` library.
#[derive(Debug, Default)]
pub struct FontSubsetter {
    // FIXME: options
}

impl FontEmbedder for FontSubsetter {
    type Error = SubsettingError;

    fn embed_font(
        &self,
        font_family: &str,
        mut used_chars: BTreeSet<char>,
    ) -> Result<EmbeddedFont, Self::Error> {
        let font_bytes = fs::read(font_family).map_err(|err| SubsettingError::Read {
            path: font_family.into(),
            err,
        })?;
        let reader = FontReader::new(&font_bytes)?;
        let font = reader.read()?;

        let permissions = font.permissions();
        if !permissions.allow_subsetting {
            return Err(SubsettingError::NoSubsetting);
        }
        if permissions.embed_only_bitmaps {
            return Err(SubsettingError::NoEmbedding);
        }

        let font_family = font
            .naming()
            .family
            .ok_or(SubsettingError::NoFontFamilyName)?;

        used_chars.remove(&'\n');
        let subset = font.subset(&used_chars)?;
        let missing_chars: String = used_chars
            .iter()
            .copied()
            .filter(|ch| !font.contains_char(*ch))
            .collect();
        if !missing_chars.is_empty() {
            return Err(SubsettingError::MissingChars(missing_chars));
        }

        let subset_bytes = subset.to_woff2();
        Ok(EmbeddedFont {
            mime_type: "font/woff2".to_owned(),
            family_name: font_family.to_owned(),
            base64_data: subset_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        svg::{Template, TemplateOptions},
        Transcript, UserInput,
    };

    #[test]
    fn subsetting_font() {
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "Hello, \u{1b}[32mworld\u{1b}[0m!",
        );

        let options = TemplateOptions {
            font_family: "../examples/RobotoMono-VariableFont_wght.ttf".to_owned(),
            ..TemplateOptions::default().with_font_subsetting(FontSubsetter::default())
        };
        let mut buffer = vec![];
        Template::new(options)
            .render(&transcript, &mut buffer)
            .unwrap();
        let buffer = String::from_utf8(buffer).unwrap();

        assert!(buffer.contains("@font-face"), "{buffer}");
        assert!(buffer.contains("font-family: \"Roboto Mono\";"), "{buffer}");
        assert!(
            buffer.contains("src: url(\"data:font/woff2;base64,"),
            "{buffer}"
        );
        assert!(
            buffer.contains("font: 14px \"Roboto Mono\", monospace;"),
            "{buffer}"
        );
    }
}
