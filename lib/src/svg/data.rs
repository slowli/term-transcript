//! Data provided to Handlebars templates.

use std::{collections::HashMap, fmt};

use serde::Serialize;

use super::write::StyledLine;
use crate::{
    svg::{EmbeddedFont, TemplateOptions},
    UserInput,
};

/// Root data structure sent to the Handlebars template.
///
/// # Examples
///
/// Here's example of JSON serialization of this type:
///
/// ```
/// # use term_transcript::{svg::{TemplateOptions, NamedPalette}, Transcript, UserInput};
/// let mut transcript = Transcript::new();
/// let input = UserInput::command("rainbow");
/// transcript.add_interaction(input, "Hello, \u{1b}[32mworld\u{1b}[0m!");
/// let template_options = TemplateOptions {
///     palette: NamedPalette::Dracula.into(),
///     font_family: "Consolas, Menlo, monospace".to_owned(),
///     ..TemplateOptions::default()
/// };
/// let data = template_options.render_data(&transcript).unwrap();
///
/// let expected_json = serde_json::json!({
///     "creator": {
///         "name": "term-transcript",
///         "version": "0.4.0",
///         "repo": "https://github.com/slowli/term-transcript",
///     },
///     "width": 720,
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
///     "window_frame": false,
///     "wrap": {
///         "hard_break_at": 80,
///     },
///     "line_numbers": null,
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
    pub(crate) output: Vec<StyledLine>,
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
