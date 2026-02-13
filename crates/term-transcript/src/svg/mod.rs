//! Provides templating logic for rendering terminal output in a visual format.
//!
//! The included templating logic allows rendering SVG images. Templating is based on [Handlebars],
//! and can be [customized](Template#customization) to support differing layout or even
//! data formats (e.g., HTML). The default template supports [a variety of options](TemplateOptions)
//! controlling output aspects, e.g. image dimensions and scrolling animation.
//!
//! [Handlebars]: https://handlebarsjs.com/
//!
//! # Examples
//!
//! See [`Template`] for examples of usage.

use std::{
    collections::{BTreeSet, HashMap},
    fmt,
    io::Write,
    iter,
};

use handlebars::{Handlebars, RenderError, RenderErrorReason, Template as HandlebarsTemplate};

#[cfg(feature = "font-subset")]
pub use self::subset::{FontFace, FontSubsetter, SubsettingError};
use self::{data::CompleteHandlebarsData, helpers::register_helpers};
pub use self::{
    data::{CreatorData, HandlebarsData, SerializedInteraction},
    font::{EmbeddedFont, EmbeddedFontFace, FontEmbedder, FontMetrics},
    options::{
        BlinkOptions, ContinuedLineNumbers, LineNumberingOptions, LineNumbers, ScrollOptions,
        TemplateOptions, ValidTemplateOptions, WindowOptions, WrapOptions,
    },
    palette::{NamedPalette, NamedPaletteParseError, Palette, TermColors},
};
use crate::{
    TermError, Transcript,
    svg::{data::StyledLine, processing::split_into_lines},
};

mod data;
mod font;
mod helpers;
mod options;
mod palette;
mod processing;
#[cfg(feature = "font-subset")]
mod subset;
#[cfg(test)]
mod tests;

const COMMON_HELPERS: &str = include_str!("common.handlebars");
const DEFAULT_TEMPLATE: &str = include_str!("default.svg.handlebars");
const PURE_TEMPLATE: &str = include_str!("pure.svg.handlebars");
const MAIN_TEMPLATE_NAME: &str = "main";

impl TemplateOptions {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(transcript), err)
    )]
    fn render_data<'s>(
        &'s self,
        transcript: &'s Transcript,
    ) -> Result<HandlebarsData<'s>, TermError> {
        let rendered_outputs = self.render_outputs(transcript);
        let mut has_failures = false;

        let mut used_chars = BTreeSet::new();
        for interaction in transcript.interactions() {
            let output = interaction.output();
            used_chars.extend(output.text().chars());

            let input = interaction.input();
            if !input.is_hidden() {
                let prompt = input.prompt();
                let input_chars = iter::once(input.as_ref())
                    .chain(prompt)
                    .flat_map(str::chars);
                used_chars.extend(input_chars);
            }
        }
        if let Some(line_numbers) = &self.line_numbers {
            used_chars.extend('0'..='9');
            let additional_chars = match &line_numbers.continued {
                ContinuedLineNumbers::Mark(mark) => mark.as_ref(),
                ContinuedLineNumbers::Inherit => "",
            };
            used_chars.extend(additional_chars.chars());
        }
        if let Some(wrap) = &self.wrap {
            let additional_chars = match wrap {
                WrapOptions::HardBreakAt { mark, .. } => mark.as_ref(),
            };
            used_chars.extend(additional_chars.chars());
        }

        let embedded_font = self
            .font_embedder
            .as_deref()
            .map(|embedder| embedder.embed_font(used_chars))
            .transpose()
            .map_err(TermError::FontEmbedding)?;

        let interactions: Vec<_> = transcript
            .interactions()
            .iter()
            .zip(rendered_outputs)
            .map(|(interaction, output)| {
                let failure = interaction
                    .exit_status()
                    .is_some_and(|status| !status.is_success());
                has_failures = has_failures || failure;
                SerializedInteraction {
                    input: interaction.input(),
                    output,
                    exit_status: interaction.exit_status().map(|status| status.0),
                    failure,
                }
            })
            .collect();

        Ok(HandlebarsData {
            creator: CreatorData::default(),
            interactions,
            options: self,
            has_failures,
            embedded_font,
        })
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", skip_all))]
    fn render_outputs<'a>(&self, transcript: &'a Transcript) -> Vec<Vec<StyledLine<'a>>> {
        let max_width = self.wrap.as_ref().map(|wrap_options| match wrap_options {
            WrapOptions::HardBreakAt { chars, .. } => chars.get(),
        });

        transcript
            .interactions()
            .iter()
            .map(|interaction| split_into_lines(interaction.output().as_str(), max_width))
            .collect()
    }
}

/// Template for rendering [`Transcript`]s, e.g. into an [SVG] image.
///
/// # Available templates
///
/// When using a template created with [`Self::new()`], a transcript is rendered into SVG
/// with the text content embedded as an HTML fragment. This is because SVG is not good
/// at laying out multiline texts and text backgrounds, while HTML excels at both.
/// As a downside of this approach, the resulting SVG requires for its viewer to support
/// HTML embedding; while web browsers *a priori* support such embedding, some other SVG viewers
/// may not.
///
/// A template created with [`Self::pure_svg()`] renders a transcript into pure SVG,
/// in which text is laid out manually. The resulting SVG is
/// supported by more viewers, but it may look incorrectly in certain corner cases.
///
/// [Snapshot testing](crate::test) functionality produces snapshots using [`Self::new()`]
/// (i.e., with HTML embedding); pure SVG templates cannot be tested.
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
/// ## Arithmetic operations: `add`, `sub`, `mul`, `div`
///
/// Perform the specified arithmetic operation on the supplied args.
/// `add` and `mul` support any number of numeric args; `sub` and `div` exactly 2 numeric args.
///
/// ```handlebars
/// {{add 2 3 5}}
/// {{div (len xs) 3}}
/// ```
///
/// ## Rounding
///
/// Rounds the provided value with a configurable number of decimal digits. Also allows specifying
/// the rounding mode: up / ceil, down / floor, or nearest / round (default).
///
/// ```handlebars
/// {{round 7.8}} {{! 8 }}
/// {{round 7.13 digits=1}} {{! 7.1 }}
/// {{round 7.13 digits=1 mode="up"}} {{! 7.2 }}
/// ```
///
/// ## Counting lines: `count_lines`
///
/// Counts the number of lines in the supplied string.
///
/// ```handlebars
/// {{count_lines test}}
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
/// ## Variable scope: `scope`, `set`
///
/// A block helper that creates a scope with variables specified in the options hash.
/// In the block, each variable can be obtained using local variable syntax (e.g., `@test`).
/// Variables can be set with the `set` helper:
///
/// - If `set` is called as a block helper, the variable is set to the contents
///   of the block, which is treated as JSON.
/// - If `set` is called as a block helper with `append=true`, then the contents of the block
///   is appended to the var, which must be a string.
/// - If the `set` helper is called as an inline helper, it sets values of the listed variables.
///
/// ```handlebars
/// {{#scope test=""}}
///   {{set test="Hello"}}
///   {{@test}} {{! Outputs `Hello` }}
///   {{#set "test"}}"{{@test}}, world!"{{/set}}
///   {{@test}} {{! Outputs `Hello, world!` }}
/// {{/scope}}
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
/// use styled_str::styled;
/// use term_transcript::{svg::*, Transcript, UserInput};
///
/// let mut transcript = Transcript::new();
/// transcript.add_interaction(
///     UserInput::command("test"),
///     styled!("Hello, [[green]]world[[/]]!").into(),
/// );
///
/// let template_options = TemplateOptions {
///     palette: NamedPalette::Dracula.into(),
///     ..TemplateOptions::default()
/// }
/// .validated()?;
/// let mut buffer = vec![];
/// Template::new(template_options).render(&transcript, &mut buffer)?;
/// let buffer = String::from_utf8(buffer)?;
/// assert!(buffer.contains(r#"Hello, <span class="fg2">world</span>!"#));
/// # anyhow::Ok(())
/// ```
pub struct Template {
    options: TemplateOptions,
    handlebars: Handlebars<'static>,
    constants: HashMap<&'static str, u32>,
}

impl fmt::Debug for Template {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Template")
            .field("options", &self.options)
            .field("constants", &self.constants)
            .finish_non_exhaustive()
    }
}

impl Default for Template {
    fn default() -> Self {
        Self::new(ValidTemplateOptions::default())
    }
}

impl Template {
    const STD_CONSTANTS: &'static [(&'static str, u32)] = &[
        ("BLOCK_MARGIN", 6),
        ("USER_INPUT_PADDING", 2),
        ("WINDOW_PADDING", 10),
        ("FONT_SIZE", 14),
        ("WINDOW_FRAME_HEIGHT", 22),
        ("LN_WIDTH", 22),
        ("LN_PADDING", 7),
        ("SCROLLBAR_RIGHT_OFFSET", 7),
    ];

    /// Initializes the default template based on provided `options`.
    #[allow(clippy::missing_panics_doc)] // Panic should never be triggered
    pub fn new(options: ValidTemplateOptions) -> Self {
        let template = HandlebarsTemplate::compile(DEFAULT_TEMPLATE)
            .expect("Default template should be valid");
        Self {
            constants: Self::STD_CONSTANTS.iter().copied().collect(),
            ..Self::custom(template, options)
        }
    }

    /// Initializes the pure SVG template based on provided `options`.
    #[allow(clippy::missing_panics_doc)] // Panic should never be triggered
    pub fn pure_svg(options: ValidTemplateOptions) -> Self {
        let template =
            HandlebarsTemplate::compile(PURE_TEMPLATE).expect("Pure template should be valid");
        Self {
            constants: Self::STD_CONSTANTS.iter().copied().collect(),
            ..Self::custom(template, options)
        }
    }

    /// Initializes a custom template.
    #[allow(clippy::missing_panics_doc)] // Panic should never be triggered
    pub fn custom(template: HandlebarsTemplate, options: ValidTemplateOptions) -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        register_helpers(&mut handlebars);
        handlebars.register_template(MAIN_TEMPLATE_NAME, template);
        let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
        handlebars.register_template("_helpers", helpers);
        Self {
            options: options.into_inner(),
            handlebars,
            constants: HashMap::new(),
        }
    }

    /// Renders the `transcript` using the template (usually as an SVG image, although
    /// custom templates may use a different output format).
    ///
    /// # Errors
    ///
    /// Returns a Handlebars rendering error, if any. Normally, the only errors could be
    /// related to I/O (e.g., the output cannot be written to a file).
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, err))]
    pub fn render<W: Write>(
        &self,
        transcript: &Transcript,
        destination: W,
    ) -> Result<(), RenderError> {
        let data = self
            .options
            .render_data(transcript)
            .map_err(|err| RenderErrorReason::NestedError(Box::new(err)))?;
        let data = CompleteHandlebarsData {
            inner: data,
            constants: &self.constants,
        };
        #[cfg(feature = "tracing")]
        tracing::debug!(?data, "using Handlebars data");

        #[cfg(feature = "tracing")]
        let _entered = tracing::debug_span!("render_to_write").entered();
        self.handlebars
            .render_to_write(MAIN_TEMPLATE_NAME, &data, destination)
    }
}
