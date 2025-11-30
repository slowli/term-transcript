//! Font embedder / subsetter based on the `font-subset` library.

use std::{collections::BTreeSet, fmt, fs, io, path::Path};

use font_subset::{FontReader, ParseError};

use super::{EmbeddedFont, FontEmbedder, FontMetrics};

/// Errors produced by [`FontSubsetter`].
#[derive(Debug)]
#[non_exhaustive]
pub enum SubsettingError {
    /// Error parsing the font file.
    Parse(ParseError),
    /// No font family name entry in the font.
    NoFontFamilyName,
    /// Subsetting is disallowed by the font permissions.
    NoSubsetting,
    /// Embedding is disallowed by the font permissions.
    NoEmbedding,
    /// The provided font is not monospace (doesn't have single glyph advance width).
    NotMonospace,
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
            Self::Parse(err) => write!(formatter, "error parsing font: {err}"),
            Self::NoFontFamilyName => formatter.write_str("no family name in font file"),
            Self::NoSubsetting => {
                formatter.write_str("subsetting is disallowed by font permissions")
            }
            Self::NoEmbedding => formatter.write_str("embedding is disallowed by font permissions"),
            Self::NotMonospace => formatter.write_str("provided font is not monospace"),
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
            Self::Parse(err) => Some(err),
            Self::NoFontFamilyName
            | Self::NoSubsetting
            | Self::NoEmbedding
            | Self::NotMonospace
            | Self::MissingChars(_) => None,
        }
    }
}

/// Font embedder / subsetter based on the `font-subset` library.
#[derive(Debug)]
pub struct FontSubsetter {
    font_bytes: Vec<u8>,
}

impl FontSubsetter {
    /// Initializes the subsetter with a font loaded from the specified path.
    ///
    /// # Errors
    ///
    /// Returns I/O errors.
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let font_bytes = fs::read(path)?;
        Ok(Self { font_bytes })
    }
}

impl FontEmbedder for FontSubsetter {
    type Error = SubsettingError;

    fn embed_font(&self, mut used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error> {
        let reader = FontReader::new(&self.font_bytes)?;
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

        let metrics = font.metrics();
        let metrics = FontMetrics {
            units_per_em: metrics.units_per_em,
            advance_width: metrics
                .monospace_advance_width
                .ok_or(SubsettingError::NotMonospace)?,
            ascent: metrics.ascent,
            descent: metrics.descent,
        };

        let subset_bytes = subset.to_woff2();
        Ok(EmbeddedFont {
            mime_type: "font/woff2".to_owned(),
            family_name: font_family.to_owned(),
            metrics,
            base64_data: subset_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use test_casing::test_casing;

    use super::*;
    use crate::{
        svg::{Template, TemplateOptions},
        Transcript, UserInput,
    };

    #[test_casing(2, [false, true])]
    fn subsetting_font(pure_svg: bool) {
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "\u{1b}[44mH\u{1b}[0mello, \u{1b}[32mworld\u{1b}[0m! ".repeat(10),
        );

        let subsetter = FontSubsetter::new("../examples/RobotoMono-VariableFont_wght.ttf").unwrap();
        let options = TemplateOptions {
            ..TemplateOptions::default().with_font_subsetting(subsetter)
        };
        let mut buffer = vec![];
        let template = if pure_svg {
            Template::pure_svg(options)
        } else {
            Template::new(options)
        };
        template.render(&transcript, &mut buffer).unwrap();
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
