//! Data provided to Handlebars templates.

use serde::Serialize;

use crate::{svg::TemplateOptions, UserInput};

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
///         "version": "0.2.0",
///         "repo": "https://github.com/slowli/term-transcript"
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
///             "white": "#e9e9f4"
///         },
///         "intense_colors": {
///             "black": "#626483",
///             "red": "#b45bcf",
///             "green": "#3a3c4e",
///             "yellow": "#4d4f68",
///             "blue": "#62d6e8",
///             "magenta": "#f1f2f8",
///             "cyan": "#00f769",
///             "white": "#f7f7fb"
///         }
///     },
///     "font_family": "Consolas, Menlo, monospace",
///     "window_frame": false,
///     "wrap": {
///         "hard_break_at": 80
///     },
///     "interactions": [{
///         "input": {
///             "text": "rainbow",
///             "prompt": "$"
///         },
///         "output_html": "Hello, <span class=\"fg2\">world</span>!"
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
}

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
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct SerializedInteraction<'a> {
    /// User's input.
    pub input: &'a UserInput,
    /// Terminal output in HTML format.
    pub output_html: String,
}
