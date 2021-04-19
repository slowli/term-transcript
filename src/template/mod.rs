use handlebars::{Context, Handlebars, Helper, HelperDef, Output, RenderContext, RenderError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{error::Error as StdError, fmt, io::Write, num::ParseIntError, str::FromStr};

use crate::{Interaction, Transcript, UserInput};

const MAIN_TEMPLATE_NAME: &str = "main";
const TEMPLATE: &str = include_str!("default.svg.handlebars");

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SvgTemplateOptions {
    pub padding: usize,
    pub font_size: usize,
    pub line_height: usize,
    pub width: usize,
    pub interaction_padding: usize,
    pub palette: Palette,
}

impl Default for SvgTemplateOptions {
    fn default() -> Self {
        Self {
            padding: 10,
            font_size: 12,
            line_height: 15,
            width: 652,
            interaction_padding: 12,
            palette: NamedPalette::Powershell.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Palette {
    pub colors: TermColors,
    pub intense_colors: TermColors,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TermColors {
    pub black: RgbColor,
    pub red: RgbColor,
    pub green: RgbColor,
    pub yellow: RgbColor,
    pub blue: RgbColor,
    pub magenta: RgbColor,
    pub cyan: RgbColor,
    pub white: RgbColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl fmt::LowerHex for RgbColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[derive(Debug)]
pub enum ColorParseError {
    NoHashPrefix,
    IncorrectLen(usize),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NamedPalette {
    Dracula,
    Powershell,
}

impl From<NamedPalette> for Palette {
    fn from(value: NamedPalette) -> Self {
        match value {
            NamedPalette::Dracula => Self {
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
            },

            NamedPalette::Powershell => Self {
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
            },
        }
    }
}

#[derive(Debug)]
pub struct SvgTemplate<'a> {
    options: SvgTemplateOptions,
    handlebars: Handlebars<'a>,
}

impl<'a> SvgTemplate<'a> {
    pub fn new(options: SvgTemplateOptions) -> Self {
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

    pub fn render<W: Write>(
        &mut self,
        transcript: &Transcript<'a>,
        destination: W,
    ) -> Result<(), RenderError> {
        #[derive(Debug, Serialize)]
        struct HandlebarsData<'r> {
            height: usize,
            interactions: Vec<SerializedInteraction<'r>>,
            #[serde(flatten)]
            options: SvgTemplateOptions,
        }

        let data = HandlebarsData {
            height: self.compute_height(transcript),
            interactions: transcript
                .interactions()
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            options: self.options,
        };
        self.handlebars
            .register_helper("content", Box::new(ContentHelper(transcript.to_owned())));
        self.handlebars
            .render_to_write(MAIN_TEMPLATE_NAME, &data, destination)
    }

    fn compute_height(&self, transcript: &Transcript<'_>) -> usize {
        let options = self.options;
        let line_count: usize = transcript
            .interactions
            .iter()
            .map(|interaction| interaction.count_lines())
            .sum();
        2 * options.padding
            + line_count * options.line_height
            + transcript.interactions.len() * options.interaction_padding
    }
}

#[derive(Debug)]
struct ContentHelper<'a>(Transcript<'a>);

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
        let interaction = self
            .0
            .interactions
            .get(index as usize)
            .ok_or_else(|| RenderError::new("index is out of bounds"))?;
        interaction
            .write_output(out)
            .map_err(|err| RenderError::from_error("content", err))
    }
}

#[derive(Debug, Serialize)]
struct SerializedInteraction<'a> {
    input: UserInput<'a>,
}

impl<'a> From<Interaction<'a>> for SerializedInteraction<'a> {
    fn from(value: Interaction<'a>) -> Self {
        Self { input: value.input }
    }
}
