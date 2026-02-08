//! Data provided to Handlebars templates.

use std::{collections::HashMap, fmt};

use anstyle::{Ansi256Color, Color, Effects, RgbColor, Style};
use serde::Serialize;

use crate::{
    UserInput,
    svg::{EmbeddedFont, TemplateOptions},
};

pub(super) mod serde_color {
    use std::fmt;

    use anstyle::RgbColor;
    use serde::{Deserializer, Serialize, Serializer, de};
    use styled_str::{parse_hex_color, rgb_color_to_hex};

    #[allow(clippy::trivially_copy_pass_by_ref)] // required by serde
    pub(crate) fn serialize<S: Serializer>(
        color: &RgbColor,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        rgb_color_to_hex(*color).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<RgbColor, D::Error> {
        #[derive(Debug)]
        struct ColorVisitor;

        impl de::Visitor<'_> for ColorVisitor {
            type Value = RgbColor;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("hex color, such as #fed or #a757ff")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                parse_hex_color(value.as_bytes()).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub(super) enum SerdeColor {
    Index(u8),
    Rgb(#[serde(with = "serde_color")] RgbColor),
}

impl From<Color> for SerdeColor {
    fn from(color: Color) -> Self {
        match color {
            Color::Ansi(color) => Self::Index(color as u8),
            Color::Ansi256(Ansi256Color(idx)) => Self::Index(idx),
            Color::Rgb(color) => Self::Rgb(color),
        }
    }
}

/// Serializable `anstyle::Style` representation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)] // makes serialization simpler
pub(super) struct SerdeStyle {
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) bold: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) italic: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) underline: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) dimmed: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) strikethrough: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) inverted: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) blink: bool,
    #[serde(skip_serializing_if = "SerdeStyle::is_false")]
    pub(super) concealed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) fg: Option<SerdeColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bg: Option<SerdeColor>,
}

impl SerdeStyle {
    #[allow(clippy::trivially_copy_pass_by_ref)] // required by serde
    fn is_false(&value: &bool) -> bool {
        !value
    }
}

impl From<Style> for SerdeStyle {
    fn from(style: Style) -> Self {
        let effects = style.get_effects();
        Self {
            bold: effects.contains(Effects::BOLD),
            italic: effects.contains(Effects::ITALIC),
            underline: effects.contains(Effects::UNDERLINE),
            dimmed: effects.contains(Effects::DIMMED),
            strikethrough: effects.contains(Effects::STRIKETHROUGH),
            inverted: effects.contains(Effects::INVERT),
            blink: effects.contains(Effects::BLINK),
            concealed: effects.contains(Effects::HIDDEN),
            fg: style.get_fg_color().map(SerdeColor::from),
            bg: style.get_bg_color().map(SerdeColor::from),
        }
    }
}

/// Serializable version of `StyledSpan`. Also, inlines text for convenience instead of using lengths.
#[derive(Debug, Serialize)]
pub(super) struct SerdeStyledSpan<'a> {
    #[serde(flatten)]
    pub(super) style: SerdeStyle,
    pub(super) text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LineBreak {
    Hard,
}

#[derive(Debug, Default, Serialize)]
pub(super) struct StyledLine<'a> {
    pub(super) spans: Vec<SerdeStyledSpan<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) br: Option<LineBreak>,
}

/// Root data structure sent to the Handlebars template.
///
/// # Examples
///
/// Here's example of JSON serialization of this type:
///
/// ```
/// use styled_str::styled;
/// # use term_transcript::{svg::{TemplateOptions, NamedPalette}, Transcript, UserInput};
///
/// let mut transcript = Transcript::new();
/// let input = UserInput::command("rainbow");
/// transcript.add_interaction(input, styled!("Hello, [[green]]world[[]]!").into());
/// let template_options = TemplateOptions {
///     palette: NamedPalette::Dracula.into(),
///     font_family: "Consolas, Menlo, monospace".to_owned(),
///     ..TemplateOptions::default()
/// }
/// .validated()?;
/// let data = template_options.render_data(&transcript).unwrap();
///
/// let expected_json = serde_json::json!({
///     "creator": {
///         "name": "term-transcript",
///         "version": "0.5.0-beta.1",
///         "repo": "https://github.com/slowli/term-transcript",
///     },
///     "width": 720,
///     "line_height": null,
///     "advance_width": null,
///     "palette": {
///         "colors": {
///             "black": "#282936",
///             "red": "#ea51b2",
///             "green": "#ebff87",
///             "yellow": "#00f769",
///             "blue": "#62d6e8",
///             "magenta": "#b45bcf",
///             "cyan": "#a1efe4",
///             "white": "#e9e9f4",
///         },
///         "intense_colors": {
///             "black": "#626483",
///             "red": "#b45bcf",
///             "green": "#3a3c4e",
///             "yellow": "#4d4f68",
///             "blue": "#62d6e8",
///             "magenta": "#f1f2f8",
///             "cyan": "#00f769",
///             "white": "#f7f7fb",
///         },
///     },
///     "font_family": "Consolas, Menlo, monospace",
///     "window": null,
///     "wrap": {
///         "hard_break_at": {
///             "chars": 80,
///             "mark": "»",
///         },
///     },
///     "line_numbers": null,
///     "dim_opacity": 0.7,
///     "blink": {
///         "opacity": 0.7,
///         "interval": 1.0,
///     },
///     "has_failures": false,
///     "interactions": [{
///         "input": {
///             "text": "rainbow",
///             "prompt": "$",
///             "hidden": false,
///         },
///         "output": [{
///             "spans": [
///                 { "text": "Hello, " },
///                 { "text": "world", "fg": 2 },
///                 { "text": "!" },
///             ],
///         }],
///         "failure": false,
///         "exit_status": null,
///     }]
/// });
/// assert_eq!(serde_json::to_value(data).unwrap(), expected_json);
/// # anyhow::Ok(())
/// ```
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct HandlebarsData<'r> {
    /// Information about the rendering software.
    pub creator: CreatorData,
    /// Template options used for rendering. These options are flattened into the parent
    /// during serialization.
    #[serde(flatten)]
    pub options: &'r TemplateOptions,
    /// Recorded terminal interactions.
    pub interactions: Vec<SerializedInteraction<'r>>,
    /// Has any of terminal interactions failed?
    pub has_failures: bool,
    /// A font (usually subset) to be embedded into the generated transcript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded_font: Option<EmbeddedFont>,
}

// 1. font-subset -> term-transcript -> font-subset-cli, ..
// Problem: different workspaces / repos; meaning that font-subset-cli will depend on 2 `font-subset`s (????)
// Patching the font-subset dep sort of works, unless term-transcript code needs to be modified
//
// 2. same, but font-subset is an optional dep in term-transcript, not used in font-subset-cli
// (replaced with a local module)

/// Information about software used for rendering (i.e., this crate).
///
/// It can make sense to include this info as a comment in the rendered template
/// for debugging purposes.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreatorData {
    /// Name of software rendering the template.
    pub name: &'static str,
    /// Version of the rendering software.
    pub version: &'static str,
    /// Link to the git repository with the rendering software.
    pub repo: &'static str,
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

/// Serializable version of [`Interaction`](crate::Interaction).
#[derive(Serialize)]
#[non_exhaustive]
pub struct SerializedInteraction<'a> {
    /// User's input.
    pub input: &'a UserInput,
    /// Terminal output in the [HTML format](#html-output).
    pub(super) output: Vec<StyledLine<'a>>,
    /// Exit status of the latest executed program, or `None` if it cannot be determined.
    pub exit_status: Option<i32>,
    /// Was execution unsuccessful judging by the [`ExitStatus`](crate::ExitStatus)?
    pub failure: bool,
}

impl fmt::Debug for SerializedInteraction<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SerializedInteraction")
            .field("input", &self.input)
            .field("output.line_count", &self.output.len())
            .field("exit_status", &self.exit_status)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CompleteHandlebarsData<'r> {
    #[serde(flatten)]
    pub inner: HandlebarsData<'r>,
    #[serde(rename = "const")]
    pub constants: &'r HashMap<&'static str, u32>,
}
