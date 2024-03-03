//! Data provided to Handlebars templates.

use serde::Serialize;

use crate::{svg::TemplateOptions, write::SvgLine, UserInput};

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
///         "version": "0.4.0-beta.1",
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
///         "output_html": "Hello, <span class=\"fg2\">world</span>!",
/// #       "output_svg": [{
/// #           "background": null,
/// #           "foreground": "Hello,\u{a0}<tspan class=\"fg2\">world</tspan>!",
/// #       }],
/// #       // ^ Implementation detail for now
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
///
/// # HTML output
///
/// An interaction contains rendered HTML for the output with styles applied
/// to the relevant segments as `<span>`s. The styles are signalled using `class`es
/// and inline `style`s:
///
/// - `fg0`–`fg15` classes specify the foreground color being
///   0th–15th [base terminal color][colors]. `fg0`–`fg7` are ordinary colors,
///   and `fg8`–`fg15` are intense variations.
/// - Likewise, `bg0`–`bg15` classes specify the background color as one of the base terminal
///   colors.
/// - Remaining indexed colors and 24-bit colors have a definite value, and thus are signalled
///   via an inline `style` (e.g., `color: #c0ffee` or `background: #c0ffee`).
/// - `bold`, `italic`, `underline`, `dimmed` classes correspond to the corresponding text styles.
/// - [Hard breaks], if they are enabled, are represented by `<b class="hard-br"><br/></b>`.
///
/// The rendered HTML is assumed to be included into a container that preserves whitespace,
/// i.e., has [`white-space`] CSS property set to `pre`. An example of such container is `<pre>`.
///
/// [colors]: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
/// [`white-space`]: https://developer.mozilla.org/en-US/docs/Web/CSS/white-space
/// [Hard breaks]: crate::svg::WrapOptions::HardBreakAt
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct SerializedInteraction<'a> {
    /// User's input.
    pub input: &'a UserInput,
    /// Terminal output in the [HTML format](#html-output).
    pub output_html: String,
    /// Terminal output in the SVG format.
    pub(crate) output_svg: Vec<SvgLine>,
    /// Exit status of the latest executed program, or `None` if it cannot be determined.
    pub exit_status: Option<i32>,
    /// Was execution unsuccessful judging by the [`ExitStatus`](crate::ExitStatus)?
    pub failure: bool,
}
