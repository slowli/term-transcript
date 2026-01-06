//! Templating-related command-line args.

use std::{
    fmt,
    fs::{self, File},
    io, mem,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::Context;
use clap::{Args, ValueEnum};
use handlebars::Template as HandlebarsTemplate;
use term_transcript::{
    svg::{
        self, BlinkOptions, FontFace, FontSubsetter, LineNumberingOptions, ScrollOptions, Template,
        TemplateOptions, ValidTemplateOptions, WrapOptions,
    },
    Transcript,
};

#[derive(Debug, Clone, ValueEnum)]
enum NamedPalette {
    Dracula,
    Powershell,
    Xterm,
    Ubuntu,
    Gjm8,
}

impl From<NamedPalette> for svg::NamedPalette {
    fn from(palette: NamedPalette) -> Self {
        match palette {
            NamedPalette::Dracula => Self::Dracula,
            NamedPalette::Powershell => Self::PowerShell,
            NamedPalette::Xterm => Self::Xterm,
            NamedPalette::Ubuntu => Self::Ubuntu,
            NamedPalette::Gjm8 => Self::Gjm8,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum LineNumbers {
    EachOutput,
    ContinuousOutputs,
    Continuous,
}

impl From<LineNumbers> for svg::LineNumbers {
    fn from(numbers: LineNumbers) -> Self {
        match numbers {
            LineNumbers::EachOutput => Self::EachOutput,
            LineNumbers::ContinuousOutputs => Self::ContinuousOutputs,
            LineNumbers::Continuous => Self::Continuous,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CssLength {
    Ems(f64),
    Pixels(f64),
}

impl CssLength {
    const FONT_SIZE: f64 = 14.0;

    fn as_ems(self) -> f64 {
        match self {
            Self::Ems(val) => val,
            Self::Pixels(val) => val / Self::FONT_SIZE,
        }
    }

    fn as_pixels(self) -> f64 {
        match self {
            Self::Ems(val) => val * Self::FONT_SIZE,
            Self::Pixels(val) => val,
        }
    }
}

impl FromStr for CssLength {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(val) = s.strip_suffix("px") {
            let val = val.trim();
            Ok(Self::Pixels(val.parse()?))
        } else if let Some(val) = s.strip_suffix("em") {
            let val = val.trim();
            Ok(Self::Ems(val.parse()?))
        } else {
            anyhow::bail!("expected value with 'px' or 'em' suffix")
        }
    }
}

impl fmt::Display for CssLength {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pixels(val) => write!(formatter, "{val}px"),
            Self::Ems(val) => write!(formatter, "{val}em"),
        }
    }
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // required by `clap`
pub(crate) struct TemplateArgs {
    /// Path to the configuration TOML file.
    ///
    /// See <https://slowli.github.io/term-transcript/term_transcript/svg/> for the configuration format.
    #[arg(
        long,
        conflicts_with_all = [
            "palette", "line_numbers", "window_frame", "additional_styles", "font_family", "width",
            "scroll", "hard_wrap", "no_wrap", "line_height", "advance_width", "dim_opacity", "blink_interval",
            "blink_opacity"
        ]
    )]
    config_path: Option<PathBuf>,
    /// Color palette to use.
    #[arg(long, short = 'p', default_value = "gjm8", value_enum)]
    palette: NamedPalette,
    /// Line numbering strategy.
    #[arg(long, short = 'n', value_enum)]
    line_numbers: Option<LineNumbers>,
    /// Mark displayed at the beginning of continued lines instead of the line number. May be empty.
    /// If not specified, continued lines will be numbered along with ordinary lines.
    #[arg(long, value_name = "MARK", requires = "line_numbers")]
    continued_mark: Option<String>,
    /// Adds a window frame around the rendered console.
    #[arg(long = "window", short = 'w')]
    window_frame: bool,
    /// CSS instructions to add at the beginning of the SVG `<style>` tag. This is mostly useful
    /// to import fonts in conjunction with `--font`.
    #[arg(long = "styles")]
    additional_styles: Option<String>,
    /// Configures a font family. The font families should be specified in the CSS format,
    /// e.g. 'Consolas, Liberation Mono'. The `monospace` fallback will be added
    /// automatically.
    #[arg(long = "font", value_name = "FONT")]
    font_family: Option<String>,
    /// Embeds the font family (after subsetting it) into the SVG file. This guarantees that
    /// the SVG will look identical on all platforms.
    #[arg(
        long,
        conflicts_with = "font_family",
        value_name = "PATH",
        value_delimiter = ':'
    )]
    embed_font: Vec<PathBuf>,
    /// Line height to use relative to the font size. If not specified, the value will be obtained
    /// from the font metrics (if a font is embedded), or set to 1.2 otherwise.
    /// Can be specified either as a fraction like 1.2, or as a value in pixels like '18px'.
    #[arg(long, value_name = "CSS_LEN")]
    line_height: Option<CssLength>,
    /// Advance width to use relative to the font size. If not specified, the value will be obtained
    /// from the font metrics (if a font is embedded), or set to 8px (~0.57em) otherwise.
    /// Can be specified either as a fraction like 0.6, or as a value in pixels like '8.5px'.
    #[arg(long, value_name = "CSS_LEN", requires = "pure_svg")]
    advance_width: Option<CssLength>,
    /// Configures width of the rendered console in SVG units. Hint: use together with `--hard-wrap $chars`,
    /// where width is around $chars * 9.
    #[arg(long, default_value_t = TemplateOptions::default().width)]
    width: NonZeroUsize,
    /// Enables scrolling animation, but only if the snapshot height exceeds a threshold height (in SVG units).
    /// If not specified, the default height is sufficient to fit 19 lines with the default template.
    #[arg(long, value_name = "HEIGHT")]
    #[allow(clippy::option_option)] // required by `clap`
    scroll: Option<Option<NonZeroUsize>>,
    /// Interval between keyframes in the scrolling animation.
    #[arg(
        long,
        value_name = "TIME",
        default_value_t = Duration::from_secs_f64(ScrollOptions::DEFAULT.interval).into(),
        requires = "scroll"
    )]
    scroll_interval: humantime::Duration,
    /// Length scrolled in each keyframe.
    #[arg(
        long,
        value_name = "CSS_LEN",
        default_value_t = CssLength::Pixels(ScrollOptions::DEFAULT.pixels_per_scroll.get() as f64),
        requires = "scroll"
    )]
    scroll_len: CssLength,
    /// Threshold to elide the penultimate scroll keyframe, relative to `scroll_len`.
    /// If the last scroll keyframe would scroll the view by less than this value (which can happen because
    /// the last scroll always aligns the scrolled view bottom with the viewport bottom), it will be
    /// combined with the penultimate keyframe.
    ///
    /// The threshold must be in [0, 1). 0 means never eliding the penultimate keyframe.
    #[arg(
        long,
        value_name = "RATIO",
        default_value_t = ScrollOptions::DEFAULT.elision_threshold,
        requires = "scroll"
    )]
    scroll_elision_threshold: f64,
    /// Opacity of dimmed text.
    #[arg(long, value_name = "RATIO", default_value_t = TemplateOptions::default().dim_opacity)]
    dim_opacity: f64,
    /// Interval between blinking animation keyframes.
    #[arg(
        long,
        value_name = "TIME",
        default_value_t = Duration::from_secs_f64(BlinkOptions::default().interval).into()
    )]
    blink_interval: humantime::Duration,
    /// Lower value of blink opacity. Must be in `[0, 1]`.
    #[arg(
        long,
        value_name = "RATIO",
        default_value_t = BlinkOptions::default().opacity
    )]
    blink_opacity: f64,
    /// Specifies text wrapping threshold in number of chars.
    #[arg(
        long = "hard-wrap",
        value_name = "CHARS",
        conflicts_with = "no_wrap",
        default_value = "80"
    )]
    hard_wrap: NonZeroUsize,
    /// Disables text wrapping (by default, text is hard-wrapped at 80 chars). Line overflows
    /// will be hidden.
    #[arg(long = "no-wrap")]
    no_wrap: bool,
    /// Employs pure SVG rendering instead of embedding HTML into SVG. Pure SVGs are supported
    /// by more viewers, but there may be rendering artifacts.
    #[arg(long = "pure-svg", conflicts_with = "template_path")]
    pure_svg: bool,
    /// Hides all user inputs; only outputs will be rendered.
    #[arg(long = "no-inputs")]
    pub(crate) no_inputs: bool,
    /// Path to a custom Handlebars template to use. `-` means not to use a template at all,
    /// and instead output JSON data that would be fed to a template.
    ///
    /// See <https://slowli.github.io/term-transcript/term_transcript/svg/> for docs on templating.
    #[arg(long = "tpl")]
    pub(crate) template_path: Option<PathBuf>,
    /// File to save the rendered SVG into. If omitted, the output will be printed to stdout.
    #[arg(long = "out", short = 'o')]
    pub(crate) out: Option<PathBuf>,
}

impl TryFrom<TemplateArgs> for TemplateOptions {
    type Error = anyhow::Error;

    fn try_from(value: TemplateArgs) -> Result<Self, Self::Error> {
        let mut this = Self {
            width: value.width,
            line_height: value.line_height.map(CssLength::as_ems),
            advance_width: value.advance_width.map(CssLength::as_ems),
            palette: svg::NamedPalette::from(value.palette).into(),
            line_numbers: value.line_numbers.map(|scope| LineNumberingOptions {
                scope: scope.into(),
                continued: value
                    .continued_mark
                    .map_or(svg::ContinuedLineNumbers::Inherit, |mark| {
                        svg::ContinuedLineNumbers::Mark(mark.into())
                    }),
            }),
            window_frame: value.window_frame,
            dim_opacity: value.dim_opacity,
            blink: BlinkOptions {
                interval: value.blink_interval.as_secs_f64(),
                opacity: value.blink_opacity,
            },
            scroll: value
                .scroll
                .map(|max_height| {
                    let mut options = ScrollOptions::default();
                    if let Some(max_height) = max_height {
                        options.max_height = max_height;
                    }
                    options.interval = value.scroll_interval.as_secs_f64();

                    let mut scroll_len = value.scroll_len.as_pixels();
                    if scroll_len.fract() != 0.0 {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(scroll_len, "scroll length is not integer, rounding");
                        scroll_len = scroll_len.round();
                    }
                    // We only check the validity of the `as usize` conversion; other checks will be performed during options validation.
                    #[allow(
                        // OK for the threshold check
                        clippy::cast_precision_loss,
                        // Doesn't happen because of previous checks
                        clippy::cast_sign_loss,
                        clippy::cast_possible_truncation
                    )]
                    {
                        anyhow::ensure!(scroll_len >= 0.0, "negative scroll length");
                        anyhow::ensure!(
                            scroll_len <= usize::MAX as f64,
                            "scroll length is too large"
                        );
                        options.pixels_per_scroll = NonZeroUsize::new(scroll_len as usize)
                            .context("scroll length must be positive")?;
                    }

                    options.elision_threshold = value.scroll_elision_threshold;
                    anyhow::Ok(options)
                })
                .transpose()?,
            wrap: if value.no_wrap {
                None
            } else {
                Some(WrapOptions::HardBreakAt(value.hard_wrap))
            },
            additional_styles: value.additional_styles.unwrap_or_default(),
            ..Self::default()
        };

        if !value.embed_font.is_empty() {
            anyhow::ensure!(
                value.embed_font.len() <= 2,
                "Only 2 fonts can be embedded at the moment (regular + bold or italic)"
            );

            let font_face = TemplateArgs::read_font_face(&value.embed_font[0])?;
            let aux_font_face = value
                .embed_font
                .get(1)
                .map(|path| TemplateArgs::read_font_face(path))
                .transpose()?;
            let subsetter = FontSubsetter::from_faces(font_face, aux_font_face)?;
            this = this.with_font_subsetting(subsetter);
        } else if let Some(mut font_family) = value.font_family {
            font_family.push_str(", monospace");
            this.font_family = font_family;
        }

        #[cfg(feature = "tracing")]
        tracing::debug!(?this, "created template options");
        Ok(this)
    }
}

impl TemplateArgs {
    #[cfg_attr(feature = "tracing", tracing::instrument(ret, err))]
    fn read_font_face(path: &Path) -> anyhow::Result<FontFace> {
        let font_bytes = fs::read(path)
            .with_context(|| format!("failed loading font from {}", path.display()))?;
        FontFace::new(font_bytes.into())
            .with_context(|| format!("invalid font at {}", path.display()))
    }

    pub(crate) fn build(mut self) -> anyhow::Result<ProcessedTemplateArgs> {
        let pure_svg = self.pure_svg;
        let out_path = mem::take(&mut self.out);
        let template_path = mem::take(&mut self.template_path);
        let config_path = mem::take(&mut self.config_path);

        let options = if let Some(path) = &config_path {
            let config = fs::read_to_string(path)
                .with_context(|| format!("cannot read TOML config from `{}`", path.display()))?;
            toml::from_str(&config).with_context(|| {
                format!("failed deserializing TOML config from `{}`", path.display())
            })?
        } else {
            TemplateOptions::try_from(self)?
        };
        let options = options
            .validated()
            .context("template options are invalid")?;

        let template = if let Some(template_path) = template_path {
            if template_path.as_os_str() == "-" {
                TemplateOrOptions::from(options)
            } else {
                let template = Self::load_template(&template_path)?;
                Template::custom(template, options).into()
            }
        } else if pure_svg {
            Template::pure_svg(options).into()
        } else {
            Template::new(options).into()
        };

        Ok(ProcessedTemplateArgs { template, out_path })
    }

    fn load_template(template_path: &Path) -> anyhow::Result<HandlebarsTemplate> {
        let template_string = fs::read_to_string(template_path).with_context(|| {
            format!(
                "cannot read Handlebars template from `{}`",
                template_path.display()
            )
        })?;
        let template = HandlebarsTemplate::compile(&template_string).with_context(|| {
            format!(
                "cannot compile Handlebars template from `{}`",
                template_path.display()
            )
        })?;
        Ok(template)
    }
}

#[derive(Debug)]
enum TemplateOrOptions {
    Template(Box<Template>),
    Options(Box<ValidTemplateOptions>),
}

impl From<Template> for TemplateOrOptions {
    fn from(template: Template) -> Self {
        Self::Template(Box::new(template))
    }
}

impl From<ValidTemplateOptions> for TemplateOrOptions {
    fn from(options: ValidTemplateOptions) -> Self {
        Self::Options(Box::new(options))
    }
}

#[derive(Debug)]
pub(crate) struct ProcessedTemplateArgs {
    template: TemplateOrOptions,
    out_path: Option<PathBuf>,
}

impl ProcessedTemplateArgs {
    pub(crate) fn render(self, transcript: &Transcript) -> anyhow::Result<()> {
        let template = match self.template {
            TemplateOrOptions::Template(template) => template,
            TemplateOrOptions::Options(options) => {
                return Self::render_data(self.out_path.as_deref(), transcript, &options);
            }
        };

        if let Some(out_path) = self.out_path {
            let out = File::create(&out_path)
                .with_context(|| format!("cannot create output file `{}`", out_path.display()))?;
            template
                .render(transcript, out)
                .with_context(|| format!("cannot render template to `{}`", out_path.display()))?;
        } else {
            template
                .render(transcript, io::stdout())
                .context("cannot render template to stdout")?;
        }
        Ok(())
    }

    fn render_data(
        out_path: Option<&Path>,
        transcript: &Transcript,
        options: &ValidTemplateOptions,
    ) -> anyhow::Result<()> {
        let data = options
            .render_data(transcript)
            .context("cannot render data for Handlebars template")?;
        if let Some(out_path) = out_path {
            let out = File::create(out_path)
                .with_context(|| format!("cannot create output file `{}`", out_path.display()))?;
            serde_json::to_writer(out, &data).with_context(|| {
                format!("cannot write Handlebars data to `{}`", out_path.display())
            })?;
        } else {
            serde_json::to_writer(io::stdout(), &data)
                .context("cannot write Handlebars data to stdout")?;
        }
        Ok(())
    }
}
