//! Data provided to Handlebars templates.

use serde::Serialize;

use std::fmt::Write;

use crate::{svg::TemplateOptions, UserInput};

const TEMPLATE: &str = include_str!("default.svg.handlebars");

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
pub struct HandlebarsData<'r, T = ()> {
    /// Information about the rendering software.
    pub creator: CreatorData,
    /// Template options used for rendering. These options are flattened into the parent
    /// during serialization.
    #[serde(flatten)]
    pub options: &'r TemplateOptions,
    /// Recorded terminal interactions.
    pub interactions: Vec<SerializedInteraction<'r>>,
    /// Template-specific data. We provide this only for the default template.
    #[serde(flatten)]
    pub(super) custom: T,
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

#[derive(Debug, Serialize)]
struct ScrollAnimationConfig {
    duration: f32,
    view_box: String,
    scrollbar_x: usize,
    scrollbar_y: String,
}

/// Custom template data for [`DefaultTemplate`].
///
/// This type is intentionally opaque; its schema is subject to change without notice.
#[derive(Debug, Serialize)]
pub struct DefaultTemplateData {
    height: usize,
    screen_height: usize,
    content_height: usize,
    scroll_animation: Option<ScrollAnimationConfig>,
}

impl DefaultTemplateData {
    /// Bottom margin for each input or output block.
    const BLOCK_MARGIN: usize = 6;
    /// Additional padding for each user input block.
    const USER_INPUT_PADDING: usize = 4;
    /// Padding within the rendered terminal window in pixels.
    const WINDOW_PADDING: usize = 10;
    /// Line height in pixels.
    pub(super) const LINE_HEIGHT: usize = 18;
    /// Height of the window frame.
    const WINDOW_FRAME_HEIGHT: usize = 22;
    /// Pixels scrolled vertically per each animation frame.
    const PIXELS_PER_SCROLL: usize = Self::LINE_HEIGHT * 4;
    /// Right offset of the scrollbar relative to the right border of the frame.
    const SCROLLBAR_RIGHT_OFFSET: usize = 7;
    /// Height of the scrollbar in pixels.
    const SCROLLBAR_HEIGHT: usize = 40;

    fn new(options: &TemplateOptions, interactions: &[SerializedInteraction<'_>]) -> Self {
        let content_height = Self::compute_content_height(interactions);
        let scroll_animation = Self::scroll_animation(options, content_height);
        let screen_height = if scroll_animation.is_some() {
            options
                .scroll
                .as_ref()
                .map_or(content_height, |scroll| scroll.max_height)
        } else {
            content_height
        };

        let mut height = screen_height + 2 * Self::WINDOW_PADDING;
        if options.window_frame {
            height += Self::WINDOW_FRAME_HEIGHT;
        }

        Self {
            height,
            screen_height,
            content_height,
            scroll_animation,
        }
    }

    fn compute_content_height(interactions: &[SerializedInteraction<'_>]) -> usize {
        let line_count: usize = interactions
            .iter()
            .map(|interaction| {
                Self::count_lines_in_input(interaction.input.as_ref())
                    + Self::count_lines_in_output(&interaction.output_html)
            })
            .sum();
        let margin_count = interactions
            .iter()
            .map(|interaction| {
                if interaction.output_html.is_empty() {
                    1
                } else {
                    2
                }
            })
            .sum::<usize>()
            .saturating_sub(1); // The last margin is not displayed.
        line_count * Self::LINE_HEIGHT
            + margin_count * Self::BLOCK_MARGIN
            + interactions.len() * Self::USER_INPUT_PADDING
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
    fn scroll_animation(
        options: &TemplateOptions,
        content_height: usize,
    ) -> Option<ScrollAnimationConfig> {
        fn div_ceil(x: usize, y: usize) -> usize {
            (x + y - 1) / y
        }

        let scroll_options = options.scroll.as_ref()?;
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
                width = options.width,
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
            scrollbar_x: options.width - Self::SCROLLBAR_RIGHT_OFFSET,
            scrollbar_y,
        })
    }
}

/// Encapsulation of a Handlebars template and logic to prepare additional inputs for it.
pub trait TemplateLogic {
    /// Custom data for the template.
    type CustomData: Serialize;

    /// Returns a Handlebars template string.
    fn template_str(&self) -> &str;

    /// Returns custom data to be embedded into the context provided to the template
    /// (i.e., [`HandlebarsData`]).
    fn custom_data(&self, data: &HandlebarsData<'_>) -> Self::CustomData;
}

/// Default template logic.
#[derive(Debug, Default)]
pub struct DefaultTemplate;

impl TemplateLogic for DefaultTemplate {
    type CustomData = DefaultTemplateData;

    fn template_str(&self) -> &str {
        TEMPLATE
    }

    fn custom_data(&self, data: &HandlebarsData<'_>) -> Self::CustomData {
        DefaultTemplateData::new(data.options, &data.interactions)
    }
}
