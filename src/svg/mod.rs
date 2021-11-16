//! Provides the SVG template for rendering terminal output in a visual format.
//!
//! # Examples
//!
//! See [`Template`] for examples of usage.

use handlebars::{Handlebars, RenderError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{
    error::Error as StdError,
    fmt::{self, Write as WriteStr},
    io::Write,
    str::FromStr,
};

pub use crate::utils::{RgbColor, RgbColorParseError};
use crate::{TermError, Transcript, UserInput};

const MAIN_TEMPLATE_NAME: &str = "main";
const TEMPLATE: &str = include_str!("default.svg.handlebars");

/// Configurable options of a [`Template`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateOptions {
    /// Width of the rendered terminal window in pixels. The default value is `720`.
    pub width: usize,
    /// Palette of terminal colors. The default value of [`Palette`] is used by default.
    pub palette: Palette,
    /// Font family specification in the CSS format. Should be monospace.
    pub font_family: String,
    /// Indicates whether to display a window frame around the shell. Default value is `false`.
    pub window_frame: bool,
    /// Options for the scroll animation. If set to `None` (which is the default),
    /// no scrolling will be enabled, and the height of the generated image is not limited.
    pub scroll: Option<ScrollOptions>,
    /// Text wrapping options. The default value of [`WrapOptions`] is used by default.
    pub wrap: Option<WrapOptions>,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            width: 720,
            palette: Palette::default(),
            font_family: "SFMono-Regular, Consolas, Liberation Mono, Menlo, monospace".to_owned(),
            window_frame: false,
            scroll: None,
            wrap: Some(WrapOptions::default()),
        }
    }
}

/// Palette of [16 standard terminal colors][colors] (8 ordinary colors + 8 intense variations).
///
/// [colors]: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Palette {
    /// Ordinary colors.
    pub colors: TermColors,
    /// Intense colors.
    pub intense_colors: TermColors,
}

/// Returns the palette specified by [`NamedPalette::Gjm8`].
impl Default for Palette {
    fn default() -> Self {
        Self::gjm8()
    }
}

impl Palette {
    const fn dracula() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x28, 0x29, 0x36),
                red: RgbColor(0xea, 0x51, 0xb2),
                green: RgbColor(0xeb, 0xff, 0x87),
                yellow: RgbColor(0x00, 0xf7, 0x69),
                blue: RgbColor(0x62, 0xd6, 0xe8),
                magenta: RgbColor(0xb4, 0x5b, 0xcf),
                cyan: RgbColor(0xa1, 0xef, 0xe4),
                white: RgbColor(0xe9, 0xe9, 0xf4),
            },
            intense_colors: TermColors {
                black: RgbColor(0x62, 0x64, 0x83),
                red: RgbColor(0xb4, 0x5b, 0xcf),
                green: RgbColor(0x3a, 0x3c, 0x4e),
                yellow: RgbColor(0x4d, 0x4f, 0x68),
                blue: RgbColor(0x62, 0xd6, 0xe8),
                magenta: RgbColor(0xf1, 0xf2, 0xf8),
                cyan: RgbColor(0x00, 0xf7, 0x69),
                white: RgbColor(0xf7, 0xf7, 0xfb),
            },
        }
    }

    const fn powershell() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x0c, 0x0c, 0x0c),
                red: RgbColor(0xc5, 0x0f, 0x1f),
                green: RgbColor(0x13, 0xa1, 0x0e),
                yellow: RgbColor(0xc1, 0x9c, 0x00),
                blue: RgbColor(0x00, 0x37, 0xda),
                magenta: RgbColor(0x88, 0x17, 0x98),
                cyan: RgbColor(0x3a, 0x96, 0xdd),
                white: RgbColor(0xcc, 0xcc, 0xcc),
            },
            intense_colors: TermColors {
                black: RgbColor(0x76, 0x76, 0x76),
                red: RgbColor(0xe7, 0x48, 0x56),
                green: RgbColor(0x16, 0xc6, 0x0c),
                yellow: RgbColor(0xf9, 0xf1, 0xa5),
                blue: RgbColor(0x3b, 0x78, 0xff),
                magenta: RgbColor(0xb4, 0x00, 0x9e),
                cyan: RgbColor(0x61, 0xd6, 0xd6),
                white: RgbColor(0xf2, 0xf2, 0xf2),
            },
        }
    }

    const fn xterm() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0, 0, 0),
                red: RgbColor(0xcd, 0, 0),
                green: RgbColor(0, 0xcd, 0),
                yellow: RgbColor(0xcd, 0xcd, 0),
                blue: RgbColor(0, 0, 0xee),
                magenta: RgbColor(0xcd, 0, 0xcd),
                cyan: RgbColor(0, 0xcd, 0xcd),
                white: RgbColor(0xe5, 0xe5, 0xe5),
            },
            intense_colors: TermColors {
                black: RgbColor(0x7f, 0x7f, 0x7f),
                red: RgbColor(0xff, 0, 0),
                green: RgbColor(0, 0xff, 0),
                yellow: RgbColor(0xff, 0xff, 0),
                blue: RgbColor(0x5c, 0x5c, 0xff),
                magenta: RgbColor(0xff, 0, 0xff),
                cyan: RgbColor(0, 0xff, 0xff),
                white: RgbColor(0xff, 0xff, 0xff),
            },
        }
    }

    const fn ubuntu() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x01, 0x01, 0x01),
                red: RgbColor(0xde, 0x38, 0x2b),
                green: RgbColor(0x38, 0xb5, 0x4a),
                yellow: RgbColor(0xff, 0xc7, 0x06),
                blue: RgbColor(0, 0x6f, 0xb8),
                magenta: RgbColor(0x76, 0x26, 0x71),
                cyan: RgbColor(0x2c, 0xb5, 0xe9),
                white: RgbColor(0xcc, 0xcc, 0xcc),
            },
            intense_colors: TermColors {
                black: RgbColor(0x80, 0x80, 0x80),
                red: RgbColor(0xff, 0, 0),
                green: RgbColor(0, 0xff, 0),
                yellow: RgbColor(0xff, 0xff, 0),
                blue: RgbColor(0, 0, 0xff),
                magenta: RgbColor(0xff, 0, 0xff),
                cyan: RgbColor(0, 0xff, 0xff),
                white: RgbColor(0xff, 0xff, 0xff),
            },
        }
    }

    const fn gjm8() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x1c, 0x1c, 0x1c),
                red: RgbColor(0xff, 0x00, 0x5b),
                green: RgbColor(0xce, 0xe3, 0x18),
                yellow: RgbColor(0xff, 0xe7, 0x55),
                blue: RgbColor(0x04, 0x8a, 0xc7),
                magenta: RgbColor(0x83, 0x3c, 0x9f),
                cyan: RgbColor(0x0a, 0xc1, 0xcd),
                white: RgbColor(0xe5, 0xe5, 0xe5),
            },
            intense_colors: TermColors {
                black: RgbColor(0x66, 0x66, 0x66),
                red: RgbColor(0xff, 0x00, 0xa0),
                green: RgbColor(0xcc, 0xff, 0x00),
                yellow: RgbColor(0xff, 0x9f, 0x00),
                blue: RgbColor(0x48, 0xc6, 0xff),
                magenta: RgbColor(0xbe, 0x67, 0xe1),
                cyan: RgbColor(0x63, 0xe7, 0xf0),
                white: RgbColor(0xf3, 0xf3, 0xf3),
            },
        }
    }
}

/// Values of [8 base terminal colors][colors].
///
/// [colors]: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TermColors {
    /// Black color.
    pub black: RgbColor,
    /// Red color.
    pub red: RgbColor,
    /// Green color.
    pub green: RgbColor,
    /// Yellow color.
    pub yellow: RgbColor,
    /// Blue color.
    pub blue: RgbColor,
    /// Magenta color.
    pub magenta: RgbColor,
    /// Cyan color.
    pub cyan: RgbColor,
    /// White color.
    pub white: RgbColor,
}

impl Serialize for RgbColor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:x}", self))
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        #[derive(Debug)]
        struct ColorVisitor;

        impl de::Visitor<'_> for ColorVisitor {
            type Value = RgbColor;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("hex color, such as #fed or #a757ff")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

/// Named [`Palette`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NamedPalette {
    /// Dracula color scheme. This is the [`Default`] value.
    Dracula,
    /// PowerShell 6 / Windows 10 console color scheme.
    PowerShell,
    /// `xterm` color scheme.
    Xterm,
    /// Ubuntu terminal color scheme.
    Ubuntu,
    /// [gjm8 color scheme](https://terminal.sexy/).
    Gjm8,
}

impl Default for NamedPalette {
    fn default() -> Self {
        Self::Gjm8
    }
}

impl From<NamedPalette> for Palette {
    fn from(value: NamedPalette) -> Self {
        match value {
            NamedPalette::Dracula => Self::dracula(),
            NamedPalette::PowerShell => Self::powershell(),
            NamedPalette::Xterm => Self::xterm(),
            NamedPalette::Ubuntu => Self::ubuntu(),
            NamedPalette::Gjm8 => Self::gjm8(),
        }
    }
}

impl FromStr for NamedPalette {
    type Err = NamedPaletteParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dracula" => Ok(Self::Dracula),
            "powershell" => Ok(Self::PowerShell),
            "xterm" => Ok(Self::Xterm),
            "ubuntu" => Ok(Self::Ubuntu),
            "gjm8" => Ok(Self::Gjm8),
            _ => Err(NamedPaletteParseError(())),
        }
    }
}

/// Errors that can occur when [parsing](FromStr) [`NamedPalette`] from a string.
#[derive(Debug)]
pub struct NamedPaletteParseError(());

impl fmt::Display for NamedPaletteParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "Invalid palette name; allowed names are `dracula`, `powershell`, `xterm`, \
             `ubuntu` and `gjm8`",
        )
    }
}

impl StdError for NamedPaletteParseError {}

/// Options that influence the scrolling animation.
///
/// The animation is only displayed if the console exceeds [`Self::max_height`]. In this case,
/// the console will be scrolled vertically with the interval of [`Self::interval`] seconds
/// between every frame. The view is moved 4 lines of text per scroll.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScrollOptions {
    /// Maximum height of the console, in pixels. The default value allows to fit 19 lines
    /// of text into the view (potentially, slightly less because of vertical margins around
    /// user inputs).
    pub max_height: usize,
    /// Interval between keyframes in seconds. The default value is `4`.
    pub interval: f32,
}

impl Default for ScrollOptions {
    fn default() -> Self {
        Self {
            max_height: Template::LINE_HEIGHT * 19,
            interval: 4.0,
        }
    }
}

/// Text wrapping options.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub enum WrapOptions {
    /// Perform a hard break at the specified width of output. The [`Default`] implementation
    /// returns this variant with width 80.
    HardBreakAt(usize),
}

impl Default for WrapOptions {
    fn default() -> Self {
        Self::HardBreakAt(80)
    }
}

/// Template for rendering [`Transcript`]s into an [SVG] image.
///
/// [SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
///
/// # Examples
///
/// ```
/// use term_transcript::{svg::*, Transcript, UserInput};
///
/// # fn main() -> anyhow::Result<()> {
/// let mut transcript = Transcript::new();
/// transcript.add_interaction(
///     UserInput::command("test"),
///     "Hello, \u{1b}[32mworld\u{1b}[0m!",
/// );
///
/// let template_options = TemplateOptions {
///     palette: NamedPalette::Dracula.into(),
///     ..TemplateOptions::default()
/// };
/// let mut buffer = vec![];
/// Template::new(template_options).render(&transcript, &mut buffer)?;
/// let buffer = String::from_utf8(buffer)?;
/// assert!(buffer.contains(r#"Hello, <span class="fg2">world</span>!"#));
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Template<'a> {
    options: TemplateOptions,
    handlebars: Handlebars<'a>,
}

impl Default for Template<'_> {
    fn default() -> Self {
        Self::new(TemplateOptions::default())
    }
}

impl<'a> Template<'a> {
    /// Bottom margin for each input or output block.
    const BLOCK_MARGIN: usize = 6;
    /// Additional padding for each user input block.
    const USER_INPUT_PADDING: usize = 4;
    /// Padding within the rendered terminal window in pixels.
    const WINDOW_PADDING: usize = 10;
    /// Line height in pixels.
    const LINE_HEIGHT: usize = 18;
    /// Height of the window frame.
    const WINDOW_FRAME_HEIGHT: usize = 22;
    /// Pixels scrolled vertically per each animation frame.
    const PIXELS_PER_SCROLL: usize = Self::LINE_HEIGHT * 4;
    /// Right offset of the scrollbar relative to the right border of the frame.
    const SCROLLBAR_RIGHT_OFFSET: usize = 7;
    /// Height of the scrollbar in pixels.
    const SCROLLBAR_HEIGHT: usize = 40;

    /// Initializes the template based on provided `options`.
    pub fn new(options: TemplateOptions) -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        handlebars
            .register_template_string(MAIN_TEMPLATE_NAME, TEMPLATE)
            .expect("Default template should be valid");

        Self {
            options,
            handlebars,
        }
    }

    /// Renders the `transcript` as an SVG image.
    ///
    /// # Errors
    ///
    /// Returns a Handlebars rendering error, if any. Normally, the only errors could be
    /// related to I/O (e.g., the image cannot be written to a file).
    pub fn render<W: Write>(
        &mut self,
        transcript: &'a Transcript,
        destination: W,
    ) -> Result<(), RenderError> {
        let rendered_outputs = self
            .render_outputs(transcript)
            .map_err(|err| RenderError::from_error("content", err))?;

        let content_height = Self::compute_content_height(transcript, &rendered_outputs);
        let scroll_animation = self.scroll_animation(content_height);
        let screen_height = if scroll_animation.is_some() {
            self.options
                .scroll
                .as_ref()
                .map_or(content_height, |scroll| scroll.max_height)
        } else {
            content_height
        };

        let mut height = screen_height + 2 * Self::WINDOW_PADDING;
        if self.options.window_frame {
            height += Self::WINDOW_FRAME_HEIGHT;
        }

        let data = HandlebarsData {
            creator: CreatorData::default(),
            height,
            content_height,
            screen_height,
            interactions: transcript
                .interactions()
                .iter()
                .zip(rendered_outputs)
                .map(|(interaction, output_html)| SerializedInteraction {
                    input: interaction.input(),
                    output_html,
                })
                .collect(),
            options: &self.options,
            scroll_animation,
        };

        self.handlebars
            .render_to_write(MAIN_TEMPLATE_NAME, &data, destination)
    }

    fn render_outputs(&self, transcript: &Transcript) -> Result<Vec<String>, TermError> {
        let max_width = self
            .options
            .wrap
            .as_ref()
            .map(|wrap_options| match wrap_options {
                WrapOptions::HardBreakAt(width) => *width,
            });

        transcript
            .interactions
            .iter()
            .map(|interaction| {
                let output = interaction.output();
                let mut buffer = String::with_capacity(output.as_ref().len());
                output.write_as_html(&mut buffer, max_width)?;
                Ok(buffer)
            })
            .collect()
    }

    fn compute_content_height(transcript: &Transcript, rendered_outputs: &[String]) -> usize {
        let line_count: usize = transcript
            .interactions
            .iter()
            .zip(rendered_outputs)
            .map(|(interaction, output_html)| {
                Self::count_lines_in_input(interaction.input().as_ref())
                    + Self::count_lines_in_output(output_html)
            })
            .sum();
        let margin_count = transcript
            .interactions
            .iter()
            .map(|interaction| {
                if interaction.output().as_ref().is_empty() {
                    1
                } else {
                    2
                }
            })
            .sum::<usize>()
            .saturating_sub(1); // The last margin is not displayed.
        line_count * Self::LINE_HEIGHT
            + margin_count * Self::BLOCK_MARGIN
            + transcript.interactions.len() * Self::USER_INPUT_PADDING
    }

    fn count_lines_in_input(input_str: &str) -> usize {
        let mut input_lines = bytecount::count(input_str.as_bytes(), b'\n');
        if !input_str.is_empty() && !input_str.ends_with('\n') {
            input_lines += 1;
        }
        input_lines
    }

    fn count_lines_in_output(output_html: &str) -> usize {
        let mut output_lines =
            bytecount::count(output_html.as_bytes(), b'\n') + output_html.matches("<br/>").count();

        if !output_html.is_empty() && !output_html.ends_with('\n') {
            output_lines += 1;
        }
        output_lines
    }

    #[allow(clippy::cast_precision_loss)] // no loss with sane amount of `steps`
    fn scroll_animation(&self, content_height: usize) -> Option<ScrollAnimationConfig> {
        fn div_ceil(x: usize, y: usize) -> usize {
            (x + y - 1) / y
        }

        let scroll_options = self.options.scroll.as_ref()?;
        let max_height = scroll_options.max_height;
        let max_offset = content_height.checked_sub(max_height)?;
        let steps = div_ceil(max_offset, Self::PIXELS_PER_SCROLL);
        debug_assert!(steps > 0);

        let mut view_box = (0..=steps).fold(String::new(), |mut acc, i| {
            let y = (Self::PIXELS_PER_SCROLL as f32 * i as f32).round();
            write!(
                &mut acc,
                "0 {y} {width} {height};",
                y = y,
                width = self.options.width,
                height = max_height
            )
            .unwrap(); // safe; writing to a string is infallible
            acc
        });
        view_box.pop(); // trim the last ';'

        let y_step = (max_height - Self::SCROLLBAR_HEIGHT) as f32 / steps as f32;
        let mut scrollbar_y = (0..=steps).fold(String::new(), |mut acc, i| {
            let y = (y_step * i as f32).round();
            write!(&mut acc, "0 {};", y).unwrap();
            acc
        });
        scrollbar_y.pop(); // trim the last ';'

        Some(ScrollAnimationConfig {
            duration: scroll_options.interval * steps as f32,
            view_box,
            scrollbar_x: self.options.width - Self::SCROLLBAR_RIGHT_OFFSET,
            scrollbar_y,
        })
    }
}

/// Root data structure sent to the Handlebars template.
#[derive(Debug, Serialize)]
struct HandlebarsData<'r> {
    creator: CreatorData,
    height: usize,
    screen_height: usize,
    content_height: usize,
    interactions: Vec<SerializedInteraction<'r>>,
    #[serde(flatten)]
    options: &'r TemplateOptions,
    scroll_animation: Option<ScrollAnimationConfig>,
}

#[derive(Debug, Serialize)]
struct CreatorData {
    name: &'static str,
    version: &'static str,
    repo: &'static str,
}

impl Default for CreatorData {
    fn default() -> Self {
        Self {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            repo: env!("CARGO_PKG_REPOSITORY"),
        }
    }
}

#[derive(Debug, Serialize)]
struct SerializedInteraction<'a> {
    input: &'a UserInput,
    output_html: String,
}

#[derive(Debug, Serialize)]
struct ScrollAnimationConfig {
    duration: f32,
    view_box: String,
    scrollbar_x: usize,
    scrollbar_y: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_simple_transcript() {
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "Hello, \u{1b}[32mworld\u{1b}[0m!",
        );

        let mut buffer = vec![];
        Template::new(TemplateOptions::default())
            .render(&transcript, &mut buffer)
            .unwrap();
        let buffer = String::from_utf8(buffer).unwrap();
        assert!(buffer.contains(r#"Hello, <span class="fg2">world</span>!"#));
        assert!(!buffer.contains("<circle"));
    }

    #[test]
    fn rendering_transcript_with_frame() {
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "Hello, \u{1b}[32mworld\u{1b}[0m!",
        );

        let mut buffer = vec![];
        let options = TemplateOptions {
            window_frame: true,
            ..TemplateOptions::default()
        };
        Template::new(options)
            .render(&transcript, &mut buffer)
            .unwrap();
        let buffer = String::from_utf8(buffer).unwrap();
        assert!(buffer.contains("<circle"));
    }

    #[test]
    fn rendering_transcript_with_animation() {
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "Hello, \u{1b}[32mworld\u{1b}[0m!\n".repeat(22),
        );

        let mut buffer = vec![];
        let options = TemplateOptions {
            scroll: Some(ScrollOptions {
                max_height: 240,
                interval: 3.0,
            }),
            ..TemplateOptions::default()
        };
        Template::new(options)
            .render(&transcript, &mut buffer)
            .unwrap();
        let buffer = String::from_utf8(buffer).unwrap();

        assert!(buffer.contains(r#"viewBox="0 0 720 260""#), "{}", buffer);
        assert!(buffer.contains("<animateTransform"), "{}", buffer);
    }

    #[test]
    fn rendering_transcript_with_wraps() {
        let mut transcript = Transcript::new();
        transcript.add_interaction(
            UserInput::command("test"),
            "Hello, \u{1b}[32mworld\u{1b}[0m!",
        );

        let mut buffer = vec![];
        let options = TemplateOptions {
            wrap: Some(WrapOptions::HardBreakAt(5)),
            ..TemplateOptions::default()
        };
        Template::new(options)
            .render(&transcript, &mut buffer)
            .unwrap();
        let buffer = String::from_utf8(buffer).unwrap();

        assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{}", buffer);
        assert!(buffer.contains("<br/>"), "{}", buffer);
    }
}
