//! Font-related functionality.

use std::{collections::BTreeSet, fmt};

use base64::{prelude::BASE64_STANDARD, Engine};
use serde::{Serialize, Serializer};

use crate::BoxedError;

/// Representation of a font that can be embedded into SVG via `@font-face` CSS with a data URL `src`.
#[derive(Debug, Serialize)]
pub struct EmbeddedFont {
    /// Family name of the font.
    pub family_name: String,
    /// Font metrics.
    pub metrics: FontMetrics,
    /// Font faces. Must have at least 1 entry.
    pub faces: Vec<EmbeddedFontFace>,
}

/// Representation of a single face of an [`EmbeddedFont`]. Corresponds to a single `@font-face` CSS rule.
#[derive(Serialize)]
pub struct EmbeddedFontFace {
    /// MIME type for the font, e.g. `font/woff2`.
    pub mime_type: String,
    /// Font data. Encoded in base64 when serialized.
    #[serde(serialize_with = "base64_encode")]
    pub base64_data: Vec<u8>,
    /// Determines the `font-weight` selector for the `@font-face` rule.
    pub is_bold: Option<bool>,
    /// Determines the `font-style` selector for the `@font-face` rule.
    pub is_italic: Option<bool>,
}

// Make `Debug` representation shorter.
impl fmt::Debug for EmbeddedFontFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedFontFace")
            .field("mime_type", &self.mime_type)
            .field("data.len", &self.base64_data.len())
            .field("is_bold", &self.is_bold)
            .field("is_italic", &self.is_italic)
            .finish()
    }
}

impl EmbeddedFontFace {
    /// Creates a face based on the provided WOFF2 font data. All selectors are set to `None`.
    pub fn woff2(data: Vec<u8>) -> Self {
        Self {
            mime_type: "font/woff2".to_owned(),
            base64_data: data,
            is_bold: None,
            is_italic: None,
        }
    }
}

fn base64_encode<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    let encoded = BASE64_STANDARD.encode(data);
    encoded.serialize(serializer)
}

/// Font metrics used in SVG layout.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FontMetrics {
    /// Font design units per em. Usually 1,000 or a power of 2 (e.g., 2,048).
    pub units_per_em: u16,
    /// Horizontal advance in font design units.
    pub advance_width: u16,
    /// Typographic ascent in font design units. Usually positive.
    pub ascent: i16,
    /// Typographic descent in font design units. Usually negative.
    pub descent: i16,
    /// `letter-spacing` adjustment for the bold font face in em units.
    pub bold_spacing: f64,
    /// `letter-spacing` adjustment for the italic font face in em units. Accounts for font advance width
    /// not matching between the regular and italic faces (e.g., in Roboto Mono), which can lead
    /// to misaligned terminal columns.
    pub italic_spacing: f64,
}

/// Produces an [`EmbeddedFont`] for SVG.
pub trait FontEmbedder: 'static + fmt::Debug + Send + Sync {
    /// Errors produced by the embedder.
    type Error: Into<BoxedError>;

    /// Performs embedding. This can involve subsetting the font based on the specified chars used in the transcript.
    ///
    /// # Errors
    ///
    /// May return errors if embedding / subsetting fails.
    fn embed_font(&self, used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error>;
}

#[derive(Debug)]
pub(super) struct BoxedErrorEmbedder<T>(pub(super) T);

impl<T: FontEmbedder> FontEmbedder for BoxedErrorEmbedder<T> {
    type Error = BoxedError;

    fn embed_font(&self, used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error> {
        self.0.embed_font(used_chars).map_err(Into::into)
    }
}
