//! Provides the SVG template for rendering terminal output in a visual format.
//!
//! # Examples
//!
//! See [`Template`] for examples of usage.

use handlebars::{Context, Handlebars, Helper, HelperDef, Output, RenderContext, RenderError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{
    convert::TryFrom,
    error::Error as StdError,
    fmt::{self, Write as WriteStr},
    io::Write,
    num::ParseIntError,
    str::FromStr,
};

use crate::{Interaction, Transcript, UserInput};

const MAIN_TEMPLATE_NAME: &str = "main";
const TEMPLATE: &str = include_str!("default.svg.handlebars");

/// Configurable options of a [`Template`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateOptions {
    /// Width of the rendered terminal window in pixels. Default value is `650`.
    pub width: usize,
    /// Palette of terminal colors.
    pub palette: Palette,
    /// Font family specification in the CSS format.
    pub font_family: String,
    /// Display window frame?
    pub window_frame: bool,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            width: 600,
            palette: NamedPalette::PowerShell.into(),
            font_family: "SFMono-Regular, Consolas, Liberation Mono, Menlo, monospace".to_owned(),
            window_frame: false,
        }
    }
}

/// Palette of 16 standard terminal colors (8 ordinary colors + 8 intense variations).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Palette {
    /// Ordinary colors.
    pub colors: TermColors,
    /// Intense colors.
    pub intense_colors: TermColors,
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

/// Values of 8 standard terminal colors.
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

/// RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl fmt::LowerHex for RgbColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Errors that can occur when parsing an [`RgbColor`] from a string.
#[derive(Debug)]
#[non_exhaustive]
pub enum ColorParseError {
    /// The color does not have `#` prefix.
    NoHashPrefix,
    /// The color has incorrect string length (not 1 or 2 chars per color channel).
    IncorrectLen(usize),
    /// Error parsing color channel value.
    IncorrectDigit(ParseIntError),
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHashPrefix => formatter.write_str("Missing '#' prefix"),
            Self::IncorrectLen(len) => write!(
                formatter,
                "Unexpected color length {}, expected 4 or 7",
                len
            ),
            Self::IncorrectDigit(err) => write!(formatter, "Error parsing hex digit: {}", err),
        }
    }
}

impl StdError for ColorParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::IncorrectDigit(err) => Some(err),
            _ => None,
        }
    }
}

impl FromStr for RgbColor {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.as_bytes()[0] != b'#' {
            Err(ColorParseError::NoHashPrefix)
        } else if s.len() == 4 {
            let r = u8::from_str_radix(&s[1..2], 16).map_err(ColorParseError::IncorrectDigit)?;
            let g = u8::from_str_radix(&s[2..3], 16).map_err(ColorParseError::IncorrectDigit)?;
            let b = u8::from_str_radix(&s[3..], 16).map_err(ColorParseError::IncorrectDigit)?;
            Ok(Self(r * 17, g * 17, b * 17))
        } else if s.len() == 7 {
            let r = u8::from_str_radix(&s[1..3], 16).map_err(ColorParseError::IncorrectDigit)?;
            let g = u8::from_str_radix(&s[3..5], 16).map_err(ColorParseError::IncorrectDigit)?;
            let b = u8::from_str_radix(&s[5..], 16).map_err(ColorParseError::IncorrectDigit)?;
            Ok(Self(r, g, b))
        } else {
            Err(ColorParseError::IncorrectLen(s.len()))
        }
    }
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
    /// Dracula color scheme.
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

/// Errors that can occur when parsing [`NamedPalette`].
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
/// assert!(buffer.contains(r#"Hello, <span class="fg-green">world</span>!"#));
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
    /// Additional padding for each user input block.
    const USER_INPUT_PADDING: usize = 12;
    /// Padding within the rendered terminal window in pixels.
    const WINDOW_PADDING: usize = 10;
    /// Line height in pixels.
    const LINE_HEIGHT: usize = 16;
    /// Height of the window frame.
    const WINDOW_FRAME_HEIGHT: usize = 22;

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
    /// Returns a rendering error, if any.
    pub fn render<W: Write>(
        &mut self,
        transcript: &'a Transcript,
        destination: W,
    ) -> Result<(), RenderError> {
        #[derive(Debug, Serialize)]
        struct HandlebarsData<'r> {
            height: usize,
            content_height: usize,
            interactions: Vec<SerializedInteraction<'r>>,
            #[serde(flatten)]
            options: &'r TemplateOptions,
        }

        let content_height = Self::compute_content_height(transcript);
        let mut height = content_height + 2 * Self::WINDOW_PADDING;
        if self.options.window_frame {
            height += Self::WINDOW_FRAME_HEIGHT;
        }

        let data = HandlebarsData {
            height,
            content_height,
            interactions: transcript.interactions().iter().map(Into::into).collect(),
            options: &self.options,
        };
        self.handlebars
            .register_helper("content", Box::new(ContentHelper(transcript)));
        self.handlebars
            .render_to_write(MAIN_TEMPLATE_NAME, &data, destination)
    }

    fn compute_content_height(transcript: &Transcript) -> usize {
        let line_count: usize = transcript
            .interactions
            .iter()
            .map(Interaction::count_lines)
            .sum();
        line_count * Self::LINE_HEIGHT + transcript.interactions.len() * Self::USER_INPUT_PADDING
    }
}

#[derive(Debug)]
struct ContentHelper<'a>(&'a Transcript);

impl HelperDef for ContentHelper<'_> {
    fn call<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'reg, 'rc>,
        _registry: &'reg Handlebars<'reg>,
        _ctx: &'rc Context,
        _render_context: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        let index = helper
            .param(0)
            .ok_or_else(|| RenderError::new("no index provided"))?;
        let index = index
            .value()
            .as_u64()
            .ok_or_else(|| RenderError::new("provided index is invalid"))?;
        let index = usize::try_from(index)
            .map_err(|err| RenderError::from_error("provided index is invalid", err))?;
        let interaction = self
            .0
            .interactions
            .get(index)
            .ok_or_else(|| RenderError::new("index is out of bounds"))?;
        interaction
            .output()
            .write_as_html(&mut OutputAdapter(out))
            .map_err(|err| RenderError::from_error("content", err))
    }
}

#[derive(Debug, Serialize)]
struct SerializedInteraction<'a> {
    input: &'a UserInput,
}

impl<'a> From<&'a Interaction> for SerializedInteraction<'a> {
    fn from(value: &'a Interaction) -> Self {
        Self {
            input: &value.input,
        }
    }
}

struct OutputAdapter<'a>(&'a mut dyn Output);

impl WriteStr for OutputAdapter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write(s).map_err(|_| fmt::Error)
    }
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
}
