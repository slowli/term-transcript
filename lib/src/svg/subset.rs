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

/// OpenType font face that can be used [for subsetting](FontSubsetter).
pub struct FontFace {
    inner: OwnedFont,
    advance_width: u16,
}

// Make `Debug` representation shorter.
impl fmt::Debug for FontFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let font = self.inner.get();
        formatter
            .debug_struct("FontFace")
            .field("family_name", &self.family_name())
            .field("category", &font.category())
            .field("variation_axes", &font.variation_axes())
            .field("metrics", &font.metrics())
            .finish_non_exhaustive()
    }
}

impl FontFace {
    /// Reads an OpenType or WOFF2 font. This is effectively a re-export from the `font-subset` crate
    /// that performs some additional checks ensuring that the font is fit for subsetting.
    ///
    /// # Errors
    ///
    /// Returns font parsing / validation errors.
    pub fn new(bytes: Box<[u8]>) -> Result<Self, SubsettingError> {
        let inner = OwnedFont::new(bytes)?;
        let advance_width = Self::check(inner.get())?;
        Ok(Self {
            inner,
            advance_width,
        })
    }

    fn check(font: &Font<'_>) -> Result<u16, SubsettingError> {
        let permissions = font.permissions();
        if !permissions.allow_subsetting {
            return Err(SubsettingError::NoSubsetting);
        }
        if permissions.embed_only_bitmaps {
            return Err(SubsettingError::NoEmbedding);
        }

        font.naming()
            .family
            .ok_or(SubsettingError::NoFontFamilyName)?;
        font.metrics()
            .monospace_advance_width
            .ok_or(SubsettingError::NotMonospace)
    }

    fn family_name(&self) -> &str {
        // `unwrap()` is safe: checked in `check()` when creating `FontFace`
        self.inner.get().naming().family.unwrap()
    }

    fn category(&self) -> FontCategory {
        self.inner.get().category()
    }

    fn metrics(&self) -> FontMetrics {
        let metrics = self.inner.get().metrics();
        FontMetrics {
            units_per_em: metrics.units_per_em,
            advance_width: self.advance_width,
            ascent: metrics.ascent,
            descent: metrics.descent,
            bold_spacing: 0.0,
            italic_spacing: 0.0,
        }
    }

    fn letter_spacing(&self, base_metrics: &FontMetrics) -> f64 {
        (f64::from(base_metrics.advance_width) - f64::from(self.advance_width))
            / f64::from(base_metrics.units_per_em)
    }

    fn checked_subset(&self, chars: &BTreeSet<char>) -> Result<Font<'_>, SubsettingError> {
        let font = self.inner.get();
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

#[derive(Debug)]
enum AuxFontFaces {
    Bold(FontFace),
    Italic(FontFace),
}

/// Font embedder / subsetter based on the `font-subset` library.
#[derive(Debug)]
pub struct FontSubsetter {
    family_name: String,
    metrics: FontMetrics,
    regular_face: FontFace,
    additional_faces: Option<AuxFontFaces>,
}

impl FontSubsetter {
    /// Initializes the subsetter with the specified font.
    ///
    /// # Arguments
    ///
    /// The font must have regular category. It may be variable (e.g., by weight).
    ///
    /// # Errors
    ///
    /// Returns an error if parsing font bytes fails.
    pub fn new(font: FontFace) -> Result<Self, SubsettingError> {
        Self::from_faces(font, None)
    }

    /// Initializes the subsetter with the specified font.
    ///
    /// # Arguments
    ///
    /// Currently supports at most 2 input fonts. These may correspond to a regular + bold or regular + italic font faces
    /// (can be provided in any order).
    /// Each of the provided fonts may be variable (e.g., by weight).
    ///
    /// # Errors
    ///
    /// Returns an error if parsing font bytes fails.
    #[cfg_attr(feature = "tracing", tracing::instrument(ret, err))]
    pub fn from_faces(
        first_face: FontFace,
        second_face: Option<FontFace>,
    ) -> Result<Self, SubsettingError> {
        use self::FontCategory::{Bold, Italic, Regular};

        let first_cat = first_face.category();
        let (regular, aux) = if let Some(second) = second_face {
            let second_cat = second.category();
            match (first_cat, second_cat) {
                (Regular, Bold) => (first_face, Some(AuxFontFaces::Bold(second))),
                (Regular, Italic) => (first_face, Some(AuxFontFaces::Italic(second))),
                (Bold, Regular) => (second, Some(AuxFontFaces::Bold(first_face))),
                (Italic, Regular) => (second, Some(AuxFontFaces::Italic(first_face))),
                _ => {
                    return Err(SubsettingError::UnsupportedFontCategories(vec![
                        first_cat, second_cat,
                    ]));
                }
            }
        } else {
            if first_cat != Regular {
                return Err(SubsettingError::UnsupportedFontCategories(vec![first_cat]));
            }
            (first_face, None)
        };

        let family_name = regular.family_name().to_owned();
        let mut metrics = regular.metrics();
        match &aux {
            Some(AuxFontFaces::Bold(font)) => {
                metrics.bold_spacing = font.letter_spacing(&metrics);
            }
            Some(AuxFontFaces::Italic(font)) => {
                metrics.italic_spacing = font.letter_spacing(&metrics);
            }
            None => { /* do nothing */ }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(?metrics, "using font metrics");

        Ok(Self {
            family_name,
            metrics,
            regular_face: regular,
            additional_faces: aux,
        })
    }
}

impl FontEmbedder for FontSubsetter {
    type Error = SubsettingError;

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self), ret, err))]
    fn embed_font(&self, mut used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error> {
        used_chars.remove(&'\n');
        let subset = self.regular_face.checked_subset(&used_chars)?;
        let mut faces = vec![EmbeddedFontFace::woff2(subset.to_woff2())];
        match &self.additional_faces {
            Some(AuxFontFaces::Bold(face)) => {
                let subset = face.checked_subset(&used_chars)?;
                faces.push(EmbeddedFontFace {
                    is_bold: Some(true),
                    ..EmbeddedFontFace::woff2(subset.to_woff2())
                });
                faces[0].is_bold = Some(false);
            }
            Some(AuxFontFaces::Italic(face)) => {
                let subset = face.checked_subset(&used_chars)?;
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
    use std::{fs, iter, path::Path};

    use assert_matches::assert_matches;
    use term_style::styled;
    use test_casing::test_casing;

    use super::*;
    use crate::{
        Transcript, UserInput,
        svg::{Template, TemplateOptions},
    };

    fn read_font(path: &str) -> FontFace {
        // Relative to the crate dir
        const FONTS_DIR: &str = "../docs/src/assets/fonts";

        let bytes = fs::read(Path::new(FONTS_DIR).join(path)).unwrap();
        FontFace::new(bytes.into()).unwrap()
    }

    fn roboto_mono() -> FontFace {
        read_font("RobotoMono.ttf")
    }

    fn roboto_mono_italic() -> FontFace {
        read_font("RobotoMono-Italic.ttf")
    }

    fn fira_mono() -> FontFace {
        read_font("FiraMono-Regular.ttf")
    }

    fn fira_mono_bold() -> FontFace {
        read_font("FiraMono-Bold.ttf")
    }

    fn test_subsetting_font(subsetter: FontSubsetter, pure_svg: bool) -> String {
        let font_family = subsetter.family_name.clone();
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            iter::repeat_n(
                styled!("[[bold on blue]]H[[]]ello, [[italic green]]world[[]]! "),
                10,
            )
            .collect(),
        );

        let options = TemplateOptions::default().with_font_subsetting(subsetter);
        let options = options.validated().unwrap();
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
        let subsetter = FontSubsetter::new(roboto_mono()).unwrap();
        assert_eq!(subsetter.family_name, "Roboto Mono");
        let buffer = test_subsetting_font(subsetter, pure_svg);

        if pure_svg {
            // Check some background boxes.
            assert!(
                buffer.contains(r#"<rect x="10" y="28.5" width="8.4" height="18.5" class="fg4"/>"#),
                "{buffer}"
            );
            assert!(
                buffer.contains(
                    r#"<rect x="127.62" y="28.5" width="8.4" height="18.5" class="fg4"/>"#
                ),
                "{buffer}"
            );
        }
    }

    #[test_casing(2, [false, true])]
    #[allow(clippy::float_cmp)] // the entire point
    fn subsetting_font_with_aux_italic_font(pure_svg: bool) {
        let subsetter =
            FontSubsetter::from_faces(roboto_mono(), Some(roboto_mono_italic())).unwrap();
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
        let subsetter = FontSubsetter::from_faces(fira_mono(), Some(fira_mono_bold())).unwrap();
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
