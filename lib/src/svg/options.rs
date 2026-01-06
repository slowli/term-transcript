//! `TemplateOptions` and related types.

use std::{borrow::Cow, num::NonZeroUsize, ops};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[cfg(feature = "font-subset")]
use super::subset::FontSubsetter;
use super::{font::BoxedErrorEmbedder, FontEmbedder, HandlebarsData, Palette};
use crate::{BoxedError, TermError, Transcript};

/// Line numbering scope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LineNumbers {
    /// Number lines in each output separately. Inputs are not numbered.
    EachOutput,
    /// Use continuous numbering for the lines in all outputs. Inputs are not numbered.
    ContinuousOutputs,
    /// Use continuous numbering for the lines in all displayed inputs (i.e., ones that
    /// are not [hidden](crate::UserInput::hide())) and outputs.
    #[default]
    Continuous,
}

/// Numbering of continued lines.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContinuedLineNumbers {
    /// Continued lines are numbered in the same way as the ordinary lines.
    #[default]
    Inherit,
    /// Mark continued lines with the specified constant string. The string may be empty.
    Mark(Cow<'static, str>),
}

impl ContinuedLineNumbers {
    /// Creates a [`Self::Mark`] variant.
    pub const fn mark(mark: &'static str) -> Self {
        Self::Mark(Cow::Borrowed(mark))
    }
}

/// Line numbering options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LineNumberingOptions {
    /// Scoping of line numbers.
    #[serde(default)]
    pub scope: LineNumbers,
    /// Numbering of continued lines.
    #[serde(default)]
    pub continued: ContinuedLineNumbers,
}

/// Configurable options of a [`Template`].
///
/// # Serialization
///
/// Options can be deserialized from `serde`-supported encoding formats, such as TOML. This is used
/// in the CLI app to read options from a file:
///
/// ```
/// # use assert_matches::assert_matches;
/// # use term_transcript::svg::{RgbColor, TemplateOptions, WrapOptions};
/// let options_toml = r#"
/// width = 900
/// window_frame = true
/// line_numbers = 'continuous'
/// wrap.hard_break_at = 100
/// scroll = { max_height = 300, pixels_per_scroll = 18, interval = 1.5 }
///
/// [palette.colors]
/// black = '#3c3836'
/// red = '#b85651'
/// green = '#8f9a52'
/// yellow = '#c18f41'
/// blue = '#68948a'
/// magenta = '#ab6c7d'
/// cyan = '#72966c'
/// white = '#a89984'
///
/// [palette.intense_colors]
/// black = '#5a524c'
/// red = '#b85651'
/// green = '#a9b665'
/// yellow = '#d8a657'
/// blue = '#7daea3'
/// magenta = '#d3869b'
/// cyan = '#89b482'
/// white = '#ddc7a1'
/// "#;
///
/// let options: TemplateOptions = toml::from_str(options_toml)?;
/// assert_eq!(options.width.get(), 900);
/// assert_matches!(
///     options.wrap,
///     Some(WrapOptions::HardBreakAt(width)) if width.get() == 100
/// );
/// assert_eq!(
///     options.palette.colors.green,
///     RgbColor(0x8f, 0x9a, 0x52)
/// );
/// # anyhow::Ok(())
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateOptions {
    /// Width of the rendered terminal window in pixels. Excludes the line numbers width if line
    /// numbering is enabled. The default value is `720`.
    #[serde(default = "TemplateOptions::default_width")]
    pub width: NonZeroUsize,
    /// Line height relative to the font size. If not specified, will be taken from font metrics (if a font is embedded),
    /// or set to 1.2 otherwise.
    pub line_height: Option<f64>,
    /// Advance width of a font relative to the font size (i.e., in em units). If not specified, will be taken from font metrics (if a font is embedded),
    /// or set to 8px (~0.57em) otherwise.
    ///
    /// For now, advance width is only applied to the pure SVG template.
    // FIXME: extract to pure SVG options?
    pub advance_width: Option<f64>,
    /// Palette of terminal colors. The default value of [`Palette`] is used by default.
    #[serde(default)]
    pub palette: Palette,
    /// Opacity of dimmed text. The default value is 0.7.
    #[serde(default = "TemplateOptions::default_dim_opacity")]
    pub dim_opacity: f64,
    /// Blink options.
    #[serde(default)]
    pub blink: BlinkOptions,
    /// CSS instructions to add at the beginning of the SVG `<style>` tag. This is mostly useful
    /// to import fonts in conjunction with `font_family`.
    ///
    /// The value is not validated in any way, so supplying invalid CSS instructions can lead
    /// to broken SVG rendering.
    #[serde(skip_serializing_if = "str::is_empty", default)]
    pub additional_styles: String,
    /// Font family specification in the CSS format. Should be monospace.
    #[serde(default = "TemplateOptions::default_font_family")]
    pub font_family: String,
    /// Indicates whether to display a window frame around the shell. Default value is `false`.
    #[serde(default)]
    pub window_frame: bool,
    /// Options for the scroll animation. If set to `None` (which is the default),
    /// no scrolling will be enabled, and the height of the generated image is not limited.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scroll: Option<ScrollOptions>,
    /// Text wrapping options. The default value of [`WrapOptions`] is used by default.
    #[serde(default = "TemplateOptions::default_wrap")]
    pub wrap: Option<WrapOptions>,
    /// Line numbering options.
    pub line_numbers: Option<LineNumberingOptions>,
    /// *Font embedder* that will embed the font into the SVG file via `@font-face` CSS.
    /// This guarantees that the SVG will look identical on all platforms.
    #[serde(skip)]
    pub font_embedder: Option<Box<dyn FontEmbedder<Error = BoxedError>>>,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            width: Self::default_width(),
            line_height: None,
            advance_width: None,
            palette: Palette::default(),
            dim_opacity: Self::default_dim_opacity(),
            blink: BlinkOptions::default(),
            additional_styles: String::new(),
            font_family: Self::default_font_family(),
            window_frame: false,
            scroll: None,
            wrap: Self::default_wrap(),
            line_numbers: None,
            font_embedder: None,
        }
    }
}

impl TemplateOptions {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.dim_opacity > 0.0 && self.dim_opacity < 1.0,
            "invalid dimmed text opacity ({:?}), should be in (0, 1)",
            self.dim_opacity
        );

        if let Some(line_height) = self.line_height {
            anyhow::ensure!(line_height > 0.0, "line_height must be positive");
            #[cfg(feature = "tracing")]
            if line_height > 2.0 {
                tracing::warn!(
                    line_height,
                    "line_height is too large, the produced SVG may look broken"
                );
            }
        }

        if let Some(advance_width) = self.advance_width {
            anyhow::ensure!(advance_width > 0.0, "advance_width must be positive");
            #[cfg(feature = "tracing")]
            if advance_width > 0.7 {
                tracing::warn!(
                    advance_width,
                    "advance_width is too large, the produced SVG may look broken"
                );
            }
            #[cfg(feature = "tracing")]
            if advance_width < 0.5 {
                tracing::warn!(
                    advance_width,
                    "advance_width is too small, the produced SVG may look broken"
                );
            }
        }

        if let Some(scroll_options) = &self.scroll {
            scroll_options
                .validate()
                .context("invalid scroll options")?;
        }

        self.blink.validate().context("invalid blink options")?;

        Ok(())
    }

    /// Sets the font embedder to be used.
    #[must_use]
    pub fn with_font_embedder(mut self, embedder: impl FontEmbedder) -> Self {
        self.font_embedder = Some(Box::new(BoxedErrorEmbedder(embedder)));
        self
    }

    /// Sets the [standard font embedder / subsetter](FontSubsetter).
    #[cfg(feature = "font-subset")]
    #[must_use]
    pub fn with_font_subsetting(self, options: FontSubsetter) -> Self {
        self.with_font_embedder(options)
    }

    const fn default_width() -> NonZeroUsize {
        NonZeroUsize::new(720).unwrap()
    }

    const fn default_dim_opacity() -> f64 {
        0.7
    }

    fn default_font_family() -> String {
        "SFMono-Regular, Consolas, Liberation Mono, Menlo, monospace".to_owned()
    }

    #[allow(clippy::unnecessary_wraps)] // required by serde
    fn default_wrap() -> Option<WrapOptions> {
        Some(WrapOptions::default())
    }

    /// Validates these options. This is equivalent to using [`TryInto`].
    ///
    /// # Errors
    ///
    /// Returns an error if options are invalid.
    pub fn validated(self) -> anyhow::Result<ValidTemplateOptions> {
        self.try_into()
    }
}

/// Options that influence the scrolling animation.
///
/// The animation is only displayed if the console exceeds [`Self::max_height`]. In this case,
/// the console will be scrolled vertically by [`Self::pixels_per_scroll`]
/// with the interval of [`Self::interval`] seconds between every frame.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ScrollOptions {
    /// Maximum height of the console, in pixels. The default value allows to fit 19 lines
    /// of text into the view with the default template (potentially, slightly less because
    /// of vertical margins around user inputs).
    #[serde(default = "ScrollOptions::default_max_height")]
    pub max_height: NonZeroUsize,
    /// Minimum scrollbar height in pixels. The default value is 14px (1em).
    #[serde(default = "ScrollOptions::default_min_scrollbar_height")]
    pub min_scrollbar_height: NonZeroUsize,
    /// Number of pixels moved each scroll. Default value is 52 (~3 lines of text with the default template).
    #[serde(default = "ScrollOptions::default_pixels_per_scroll")]
    pub pixels_per_scroll: NonZeroUsize,
    /// Interval between keyframes in seconds. The default value is `4`.
    #[serde(default = "ScrollOptions::default_interval")]
    pub interval: f64,
    /// Threshold to elide the penultimate scroll keyframe, relative to `pixels_per_scroll`.
    /// If the last scroll keyframe would scroll the view by less than this value (which can happen because
    /// the last scroll always aligns the scrolled view bottom with the viewport bottom), it will be
    /// combined with the penultimate keyframe.
    ///
    /// The threshold must be in [0, 1). 0 means never eliding the penultimate keyframe. The default value is 0.25.
    #[serde(default = "ScrollOptions::default_elision_threshold")]
    pub elision_threshold: f64,
}

impl Default for ScrollOptions {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ScrollOptions {
    /// Default options.
    pub const DEFAULT: Self = Self {
        max_height: Self::default_max_height(),
        min_scrollbar_height: Self::default_min_scrollbar_height(),
        pixels_per_scroll: Self::default_pixels_per_scroll(),
        interval: Self::default_interval(),
        elision_threshold: Self::default_elision_threshold(),
    };

    const fn default_max_height() -> NonZeroUsize {
        NonZeroUsize::new(18 * 19).unwrap()
    }

    const fn default_min_scrollbar_height() -> NonZeroUsize {
        NonZeroUsize::new(14).unwrap()
    }

    const fn default_pixels_per_scroll() -> NonZeroUsize {
        NonZeroUsize::new(52).unwrap()
    }

    const fn default_interval() -> f64 {
        4.0
    }

    const fn default_elision_threshold() -> f64 {
        0.25
    }

    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.interval > 0.0, "interval must be positive");
        anyhow::ensure!(
            self.elision_threshold >= 0.0 && self.elision_threshold < 1.0,
            "elision_threshold must be in [0, 1)"
        );

        anyhow::ensure!(
            self.min_scrollbar_height < self.max_height,
            "min_scrollbar_height={} must be lesser than max_height={}",
            self.min_scrollbar_height,
            self.max_height
        );
        Ok(())
    }
}

/// Text wrapping options.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum WrapOptions {
    /// Perform a hard break at the specified width of output. The [`Default`] implementation
    /// returns this variant with width 80.
    HardBreakAt(NonZeroUsize),
}

impl Default for WrapOptions {
    fn default() -> Self {
        Self::HardBreakAt(NonZeroUsize::new(80).unwrap())
    }
}

/// Blink options.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlinkOptions {
    /// Interval between blinking animation keyframes in seconds.
    #[serde(default = "BlinkOptions::default_interval")]
    pub interval: f64,
    /// Lower value of blink opacity. Must be in `[0, 1]`.
    #[serde(default = "TemplateOptions::default_dim_opacity")]
    pub opacity: f64,
}

impl Default for BlinkOptions {
    fn default() -> Self {
        Self {
            interval: Self::default_interval(),
            opacity: TemplateOptions::default_dim_opacity(),
        }
    }
}

impl BlinkOptions {
    const fn default_interval() -> f64 {
        1.0
    }

    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.interval > 0.0, "interval must be positive");
        anyhow::ensure!(
            self.opacity >= 0.0 && self.opacity <= 1.0,
            "opacity must be in [0, 1]"
        );
        Ok(())
    }
}

/// Valid wrapper for [`TemplateOptions`]. The only way to construct this wrapper is to convert
/// [`TemplateOptions`] via [`validated()`](TemplateOptions::validated()) or [`TryInto`].
#[derive(Debug, Default)]
pub struct ValidTemplateOptions(TemplateOptions);

impl ops::Deref for ValidTemplateOptions {
    type Target = TemplateOptions;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<TemplateOptions> for ValidTemplateOptions {
    type Error = anyhow::Error;

    fn try_from(options: TemplateOptions) -> Result<Self, Self::Error> {
        options.validate()?;
        Ok(Self(options))
    }
}

impl ValidTemplateOptions {
    /// Generates data for rendering.
    ///
    /// # Errors
    ///
    /// Returns an error if output cannot be rendered to HTML (e.g., it contains invalid
    /// SGR sequences).
    pub fn render_data<'s>(
        &'s self,
        transcript: &'s Transcript,
    ) -> Result<HandlebarsData<'s>, TermError> {
        self.0.render_data(transcript)
    }

    /// Unwraps the contained options.
    pub fn into_inner(self) -> TemplateOptions {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_scroll_options() {
        let json = serde_json::json!({});
        let options: ScrollOptions = serde_json::from_value(json).unwrap();
        assert_eq!(options, ScrollOptions::DEFAULT);

        let json = serde_json::json!({
            "pixels_per_scroll": 40,
            "elision_threshold": 0.1,
        });
        let options: ScrollOptions = serde_json::from_value(json).unwrap();
        assert_eq!(
            options,
            ScrollOptions {
                pixels_per_scroll: NonZeroUsize::new(40).unwrap(),
                elision_threshold: 0.1,
                ..ScrollOptions::DEFAULT
            }
        );
    }

    #[test]
    fn validating_options() {
        // Default options must be valid.
        TemplateOptions::default().validate().unwrap();

        let bogus_options = TemplateOptions {
            line_height: Some(-1.0),
            ..TemplateOptions::default()
        };
        let err = bogus_options.validate().unwrap_err().to_string();
        assert!(err.contains("line_height"), "{err}");

        let bogus_options = TemplateOptions {
            advance_width: Some(-1.0),
            ..TemplateOptions::default()
        };
        let err = bogus_options.validate().unwrap_err().to_string();
        assert!(err.contains("advance_width"), "{err}");

        let bogus_options = TemplateOptions {
            scroll: Some(ScrollOptions {
                interval: -1.0,
                ..ScrollOptions::default()
            }),
            ..TemplateOptions::default()
        };
        let err = format!("{:#}", bogus_options.validate().unwrap_err());
        assert!(err.contains("interval"), "{err}");

        for elision_threshold in [-1.0, 1.0] {
            let bogus_options = TemplateOptions {
                scroll: Some(ScrollOptions {
                    elision_threshold,
                    ..ScrollOptions::default()
                }),
                ..TemplateOptions::default()
            };
            let err = format!("{:#}", bogus_options.validate().unwrap_err());
            assert!(err.contains("elision_threshold"), "{err}");
        }
    }
}
