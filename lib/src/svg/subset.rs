//! Font embedder / subsetter based on the `font-subset` library.

use std::{collections::BTreeSet, fmt};

use font_subset::{Font, FontCategory, OwnedFont, ParseError};

use super::{EmbeddedFont, EmbeddedFontFace, FontEmbedder, FontMetrics};

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
    /// Unsupported font categories in the provided font faces.
    UnsupportedFontCategories(Vec<FontCategory>),
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
            Self::UnsupportedFontCategories(categories) => {
                write!(
                    formatter,
                    "unsupported font categories in the provided font faces: {categories:?}"
                )
            }
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
            | Self::UnsupportedFontCategories(_)
            | Self::MissingChars(_) => None,
        }
    }
}

#[derive(Debug)]
enum AuxFontFaces {
    Bold(OwnedFont),
    Italic(OwnedFont),
}

/// Font embedder / subsetter based on the `font-subset` library.
#[derive(Debug)]
pub struct FontSubsetter {
    family_name: String,
    metrics: FontMetrics,
    regular_face: OwnedFont,
    additional_faces: Option<AuxFontFaces>,
}

impl FontSubsetter {
    /// Initializes the subsetter with the specified font bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing font bytes fails.
    pub fn new(
        font_bytes: Box<[u8]>,
        second_face: Option<Box<[u8]>>,
    ) -> Result<Self, SubsettingError> {
        use self::FontCategory::{Bold, Italic, Regular};

        let first_face = OwnedFont::new(font_bytes)?;
        let family_name = Self::check(first_face.get())?.to_owned();
        let mut metrics = Self::convert_metrics(&first_face.get().metrics())?;

        let second_face = second_face
            .map(|bytes| {
                let font = OwnedFont::new(bytes)?;
                Self::check(font.get())?;
                Ok::<_, SubsettingError>(font)
            })
            .transpose()?;

        let first_cat = first_face.get().category();
        let (regular, aux) = if let Some(second) = second_face {
            let second_cat = second.get().category();
            match (first_cat, second_cat) {
                (Regular, Bold) => (first_face, Some(AuxFontFaces::Bold(second))),
                (Regular, Italic) => (first_face, Some(AuxFontFaces::Italic(second))),
                (Bold, Regular) => (second, Some(AuxFontFaces::Bold(first_face))),
                (Italic, Regular) => (second, Some(AuxFontFaces::Italic(first_face))),
                _ => {
                    return Err(SubsettingError::UnsupportedFontCategories(vec![
                        first_cat, second_cat,
                    ]))
                }
            }
        } else {
            if first_cat != Regular {
                return Err(SubsettingError::UnsupportedFontCategories(vec![first_cat]));
            }
            (first_face, None)
        };

        match &aux {
            Some(AuxFontFaces::Bold(font)) => {
                metrics.bold_spacing = Self::letter_spacing(&metrics, font.get())?;
            }
            Some(AuxFontFaces::Italic(font)) => {
                metrics.italic_spacing = Self::letter_spacing(&metrics, font.get())?;
            }
            None => { /* do nothing */ }
        }

        Ok(Self {
            family_name,
            metrics,
            regular_face: regular,
            additional_faces: aux,
        })
    }

    fn convert_metrics(metrics: &font_subset::FontMetrics) -> Result<FontMetrics, SubsettingError> {
        Ok(FontMetrics {
            units_per_em: metrics.units_per_em,
            advance_width: metrics
                .monospace_advance_width
                .ok_or(SubsettingError::NotMonospace)?,
            ascent: metrics.ascent,
            descent: metrics.descent,
            bold_spacing: 0.0,
            italic_spacing: 0.0,
        })
    }

    fn letter_spacing(base_metrics: &FontMetrics, font: &Font<'_>) -> Result<f64, SubsettingError> {
        let aux_advance_width = font
            .metrics()
            .monospace_advance_width
            .ok_or(SubsettingError::NotMonospace)?;
        let aux_advance_width = f64::from(aux_advance_width);
        Ok((f64::from(base_metrics.advance_width) - aux_advance_width)
            / f64::from(base_metrics.units_per_em))
    }

    /// Returns the font family name.
    fn check<'font>(font: &'font Font<'_>) -> Result<&'font str, SubsettingError> {
        let permissions = font.permissions();
        if !permissions.allow_subsetting {
            return Err(SubsettingError::NoSubsetting);
        }
        if permissions.embed_only_bitmaps {
            return Err(SubsettingError::NoEmbedding);
        }

        font.naming()
            .family
            .ok_or(SubsettingError::NoFontFamilyName)
    }

    fn checked_subset<'a>(
        font: &Font<'a>,
        chars: &BTreeSet<char>,
    ) -> Result<Font<'a>, SubsettingError> {
        let missing_chars: String = chars
            .iter()
            .copied()
            .filter(|ch| !font.contains_char(*ch))
            .collect();
        if !missing_chars.is_empty() {
            return Err(SubsettingError::MissingChars(missing_chars));
        }
        font.subset(chars).map_err(Into::into)
    }
}

impl FontEmbedder for FontSubsetter {
    type Error = SubsettingError;

    fn embed_font(&self, mut used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error> {
        used_chars.remove(&'\n');
        let subset = Self::checked_subset(self.regular_face.get(), &used_chars)?;
        let mut faces = vec![EmbeddedFontFace::woff2(subset.to_woff2())];
        match &self.additional_faces {
            Some(AuxFontFaces::Bold(face)) => {
                let subset = Self::checked_subset(face.get(), &used_chars)?;
                faces.push(EmbeddedFontFace {
                    is_bold: Some(true),
                    ..EmbeddedFontFace::woff2(subset.to_woff2())
                });
                faces[0].is_bold = Some(false);
            }
            Some(AuxFontFaces::Italic(face)) => {
                let subset = Self::checked_subset(face.get(), &used_chars)?;
                faces.push(EmbeddedFontFace {
                    is_italic: Some(true),
                    ..EmbeddedFontFace::woff2(subset.to_woff2())
                });
                faces[0].is_italic = Some(false);
            }
            None => { /* do nothing */ }
        }

        Ok(EmbeddedFont {
            family_name: self.family_name.clone(),
            metrics: self.metrics,
            faces,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_matches::assert_matches;
    use test_casing::test_casing;

    use super::*;
    use crate::{
        svg::{Template, TemplateOptions},
        Transcript, UserInput,
    };

    fn roboto_mono() -> Box<[u8]> {
        fs::read("../examples/fonts/RobotoMono-VariableFont_wght.ttf")
            .unwrap()
            .into()
    }

    fn roboto_mono_italic() -> Box<[u8]> {
        fs::read("../examples/fonts/RobotoMono-Italic-VariableFont_wght.ttf")
            .unwrap()
            .into()
    }

    fn fira_mono() -> Box<[u8]> {
        fs::read("../examples/fonts/FiraMono-Regular.ttf")
            .unwrap()
            .into()
    }

    fn fira_mono_bold() -> Box<[u8]> {
        fs::read("../examples/fonts/FiraMono-Bold.ttf")
            .unwrap()
            .into()
    }

    fn test_subsetting_font(subsetter: FontSubsetter, pure_svg: bool) -> String {
        let font_family = subsetter.family_name.clone();
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "\u{1b}[44m\u{1b}[1mH\u{1b}[0mello, \u{1b}[32m\u{1b}[3mworld\u{1b}[0m! ".repeat(10),
        );

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
        assert!(
            buffer.contains(&format!("font-family: \"{font_family}\";")),
            "{buffer}"
        );
        assert!(
            buffer.contains("src: url(\"data:font/woff2;base64,"),
            "{buffer}"
        );
        assert!(
            buffer.contains(&format!("font: 14px \"{font_family}\", monospace;")),
            "{buffer}"
        );

        buffer
    }

    #[test_casing(2, [false, true])]
    fn subsetting_font(pure_svg: bool) {
        let subsetter = FontSubsetter::new(roboto_mono(), None).unwrap();
        assert_eq!(subsetter.family_name, "Roboto Mono");
        let buffer = test_subsetting_font(subsetter, pure_svg);

        if pure_svg {
            // Check some background boxes.
            assert!(
                buffer.contains(
                    r#"<rect x="10.0" y="27.33" width="8.4" height="18.46" class="fg4"/>"#
                ),
                "{buffer}"
            );
            assert!(
                buffer.contains(
                    r#"<rect x="127.62" y="27.33" width="8.4" height="18.46" class="fg4"/>"#
                ),
                "{buffer}"
            );
        }
    }

    #[test_casing(2, [false, true])]
    #[allow(clippy::float_cmp)] // the entire point
    fn subsetting_font_with_aux_italic_font(pure_svg: bool) {
        let subsetter = FontSubsetter::new(roboto_mono(), Some(roboto_mono_italic())).unwrap();
        assert_eq!(subsetter.family_name, "Roboto Mono");
        assert_matches!(&subsetter.additional_faces, Some(AuxFontFaces::Italic(_)));
        assert_eq!(subsetter.metrics.bold_spacing, 0.0);
        assert_ne!(subsetter.metrics.italic_spacing, 0.0);

        let buffer = test_subsetting_font(subsetter, pure_svg);
        let font_faces = buffer
            .lines()
            .filter(|line| line.trim_start().starts_with("@font-face"))
            .count();
        assert_eq!(font_faces, 2, "{buffer}");

        assert!(
            buffer.contains(".bold,.prompt { font-weight: bold; }"),
            "{buffer}"
        );
        assert!(
            buffer.contains(".italic { font-style: italic; letter-spacing: 0.0132em; }"),
            "{buffer}"
        );
    }

    #[test_casing(2, [false, true])]
    #[allow(clippy::float_cmp)] // the entire point
    fn subsetting_font_with_aux_bold_font(pure_svg: bool) {
        let subsetter = FontSubsetter::new(fira_mono(), Some(fira_mono_bold())).unwrap();
        assert_eq!(subsetter.family_name, "Fira Mono");
        assert_matches!(&subsetter.additional_faces, Some(AuxFontFaces::Bold(_)));
        // Fira Mono Bold has the same advance width as the regular font face
        assert_eq!(subsetter.metrics.bold_spacing, 0.0);
        assert_eq!(subsetter.metrics.italic_spacing, 0.0);

        let buffer = test_subsetting_font(subsetter, pure_svg);
        let font_faces = buffer
            .lines()
            .filter(|line| line.trim_start().starts_with("@font-face"))
            .count();
        assert_eq!(font_faces, 2, "{buffer}");

        assert!(
            buffer.contains(".bold,.prompt { font-weight: bold; }"),
            "{buffer}"
        );
        assert!(
            buffer.contains(".italic { font-style: italic; }"),
            "{buffer}"
        );
    }
}
