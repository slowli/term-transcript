//! Provides templating logic for rendering terminal output in a visual format.
//!
//! The included templating logic allows rendering SVG images. Templating is based on [Handlebars],
//! and can be [customized](Template#customization) to support differing layout or even
//! data formats (e.g., HTML).
//!
//! [Handlebars]: https://handlebarsjs.com/
//!
//! # Examples
//!
//! See [`Template`] for examples of usage.

use handlebars::{Handlebars, RenderError, Template as HandlebarsTemplate};
use serde::{Deserialize, Serialize};

use std::io::Write;

mod data;
mod helpers;
mod palette;

pub use self::{
    data::{CreatorData, HandlebarsData, SerializedInteraction},
    palette::{NamedPalette, NamedPaletteParseError, Palette, TermColors},
};
pub use crate::utils::{RgbColor, RgbColorParseError};

use self::helpers::register_helpers;
use crate::{TermError, Transcript};

const DEFAULT_TEMPLATE: &str = include_str!("default.svg.handlebars");
const MAIN_TEMPLATE_NAME: &str = "main";

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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scroll: Option<ScrollOptions>,
    /// Text wrapping options. The default value of [`WrapOptions`] is used by default.
    #[serde(skip_serializing_if = "Option::is_none", default)]
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

impl TemplateOptions {
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
        let rendered_outputs = self.render_outputs(transcript)?;

        let interactions: Vec<_> = transcript
            .interactions()
            .iter()
            .zip(rendered_outputs)
            .map(|(interaction, output_html)| SerializedInteraction {
                input: interaction.input(),
                output_html,
            })
            .collect();

        Ok(HandlebarsData {
            creator: CreatorData::default(),
            interactions,
            options: self,
        })
    }

    fn render_outputs(&self, transcript: &Transcript) -> Result<Vec<String>, TermError> {
        let max_width = self.wrap.as_ref().map(|wrap_options| match wrap_options {
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
}

/// Options that influence the scrolling animation.
///
/// The animation is only displayed if the console exceeds [`Self::max_height`]. In this case,
/// the console will be scrolled vertically with the interval of [`Self::interval`] seconds
/// between every frame. The view is moved 4 lines of text per scroll.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScrollOptions {
    /// Maximum height of the console, in pixels. The default value allows to fit 19 lines
    /// of text into the view with the default template (potentially, slightly less because
    /// of vertical margins around user inputs).
    pub max_height: usize,
    /// Interval between keyframes in seconds. The default value is `4`.
    pub interval: f32,
}

impl Default for ScrollOptions {
    fn default() -> Self {
        const DEFAULT_LINE_HEIGHT: usize = 18; // from the default template
        Self {
            max_height: DEFAULT_LINE_HEIGHT * 19,
            interval: 4.0,
        }
    }
}

/// Text wrapping options.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
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

/// Template for rendering [`Transcript`]s, e.g. into an [SVG] image.
///
/// # Customization
///
/// A custom [Handlebars] template can be supplied via [`Self::custom()`]. This can be used
/// to partially or completely change rendering logic, including the output format (e.g.,
/// to render to HTML instead of SVG).
///
/// Data supplied to a template is [`HandlebarsData`].
///
/// Besides [built-in Handlebars helpers][rust-helpers] (a superset of [standard helpers]),
/// custom templates have access to the following additional helpers. All the helpers are
/// extensively used by the [default template]; thus, studying it may be a good place to start
/// customizing. Another example is an [HTML template] from the crate examples.
///
/// ## Arithmetic helpers: `add`, `sub`, `mul`, `div`
///
/// Perform the specified arithmetic operation on the supplied args.
/// `add` and `mul` support any number of numeric args; `sub` and `div` exactly 2 numeric args.
/// `div` also supports rounding via `round` hash option. `round=true` rounds to the nearest
/// integer; `round="up"` / `round="down"` perform rounding in the specified direction.
///
/// ```handlebars
/// {{add 2 3 5}}
/// {{div (len xs) 3 round="up"}}
/// ```
///
/// ## Counting lines: `count_lines`
///
/// Counts the number of lines in the supplied string. If `format="html"` hash option is included,
/// line breaks introduced by `<br/>` tags are also counted.
///
/// ```handlebars
/// {{count_lines test}}
/// {{count_lines test format="html"}}
/// ```
///
/// ## Integer ranges: `range`
///
/// Creates an array with integers in the range specified by the 2 provided integer args.
/// The "from" bound is inclusive, the "to" one is exclusive.
///
/// ```handlebars
/// {{#each (range 0 3)}}{{@index}}, {{/each}}
/// {{! Will output `0, 1, 2,` }}
/// ```
///
/// ## Variable scope: `scope`
///
/// A block helper that creates a scope with variables specified in the options hash.
/// In the block, each variable can be obtained or set using an eponymous helper:
///
/// - If the variable helper is called as a block helper, the variable is set to the contents
///   of the block, which is treated as JSON.
/// - If the variable helper is called as an inline helper with the `set` option, the variable
///   is set to the value of the option.
/// - Otherwise, the variable helper acts as a getter for the current value of the variable.
///
/// ```handlebars
/// {{#scope test=""}}
///   {{test set="Hello"}}
///   {{test}} {{! Outputs `Hello` }}
///   {{#test}}{{test}}, world!{{/test}}
///   {{test}} {{! Outputs `Hello, world!` }}
/// {{/scope}}
/// ```
///
/// Since variable getters are helpers, not "real" variables, they should be enclosed
/// in parentheses `()` if used as args / options for other helpers, e.g. `{{add (test) 2}}`.
///
/// ## Partial evaluation: `eval`
///
/// Evaluates a partial with the provided name and parses its output as JSON. This can be used
/// to define "functions" for better code structuring. Function args can be supplied in options
/// hash.
///
/// ```handlebars
/// {{#*inline "some_function"}}
///   {{add x y}}
/// {{/inline}}
/// {{#with (eval "some_function" x=3 y=5) as |sum|}}
///   {{sum}} {{! Outputs 8 }}
/// {{/with}}
/// ```
///
/// [SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
/// [Handlebars]: https://handlebarsjs.com/
/// [rust-helpers]: https://docs.rs/handlebars/latest/handlebars/index.html#built-in-helpers
/// [standard helpers]: https://handlebarsjs.com/guide/builtin-helpers.html
/// [default template]: https://github.com/slowli/term-transcript/blob/master/src/svg/default.svg.handlebars
/// [HTML template]: https://github.com/slowli/term-transcript/blob/master/examples/custom.html.handlebars
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
pub struct Template {
    options: TemplateOptions,
    handlebars: Handlebars<'static>,
}

impl Default for Template {
    fn default() -> Self {
        Self::new(TemplateOptions::default())
    }
}

impl Template {
    /// Initializes the default template based on provided `options`.
    pub fn new(options: TemplateOptions) -> Self {
        let template = HandlebarsTemplate::compile(DEFAULT_TEMPLATE)
            .expect("Default template should be valid");
        Self::custom(template, options)
    }

    /// Initializes a custom template.
    pub fn custom(template: HandlebarsTemplate, options: TemplateOptions) -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        register_helpers(&mut handlebars);
        handlebars.register_template(MAIN_TEMPLATE_NAME, template);
        Self {
            options,
            handlebars,
        }
    }

    /// Renders the `transcript` using the template (usually as an SVG image, although templates
    /// may use different formats).
    ///
    /// # Errors
    ///
    /// Returns a Handlebars rendering error, if any. Normally, the only errors could be
    /// related to I/O (e.g., the image cannot be written to a file).
    pub fn render<W: Write>(
        &self,
        transcript: &Transcript,
        destination: W,
    ) -> Result<(), RenderError> {
        let data = self
            .options
            .render_data(transcript)
            .map_err(|err| RenderError::from_error("content", err))?;
        let data = HandlebarsData {
            creator: data.creator,
            options: data.options,
            interactions: data.interactions,
        };
        self.handlebars
            .render_to_write(MAIN_TEMPLATE_NAME, &data, destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UserInput;

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
        assert!(buffer.starts_with("<!--"));
        assert!(buffer.ends_with("</svg>\n"), "{}", buffer);
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
