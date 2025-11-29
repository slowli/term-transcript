//! Templating-related command-line args.

use std::{
    fs::{self, File},
    io, mem,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{Args, ValueEnum};
use handlebars::Template as HandlebarsTemplate;
use term_transcript::{
    svg::{self, FontSubsetter, ScrollOptions, Template, TemplateOptions, WrapOptions},
    Transcript, UserInput,
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

#[derive(Debug, Args)]
pub(crate) struct TemplateArgs {
    /// Path to the configuration TOML file.
    ///
    /// See https://slowli.github.io/term-transcript/term_transcript/svg/ for the configuration format.
    #[arg(
        long,
        conflicts_with_all = [
            "palette", "line_numbers", "window_frame", "additional_styles", "font_family", "width",
            "scroll", "hard_wrap", "no_wrap",
        ]
    )]
    config_path: Option<PathBuf>,
    /// Color palette to use.
    #[arg(long, short = 'p', default_value = "gjm8", value_enum)]
    palette: NamedPalette,
    /// Line numbering strategy.
    #[arg(long, short = 'n', value_enum)]
    line_numbers: Option<LineNumbers>,
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
    #[arg(long, conflicts_with = "font_family", value_name = "PATH")]
    embed_font: Option<PathBuf>,
    /// Configures width of the rendered console in SVG units. Hint: use together with `--hard-wrap $chars`,
    /// where width is around $chars * 9.
    #[arg(long, default_value = "720")]
    width: usize,
    /// Enables scrolling animation, but only if the snapshot height exceeds a threshold height (in SVG units).
    /// If not specified, the default height is sufficient to fit 19 lines with the default template.
    #[arg(long, value_name = "HEIGHT")]
    scroll: Option<Option<usize>>,
    /// Specifies text wrapping threshold in number of chars.
    #[arg(
        long = "hard-wrap",
        value_name = "CHARS",
        conflicts_with = "no_wrap",
        default_value = "80"
    )]
    hard_wrap: usize,
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
    no_inputs: bool,
    /// Path to a custom Handlebars template to use. `-` means not to use a template at all,
    /// and instead output JSON data that would be fed to a template.
    ///
    /// See https://slowli.github.io/term-transcript/term_transcript/svg/ for docs on templating.
    #[arg(long = "tpl")]
    template_path: Option<PathBuf>,
    /// File to save the rendered SVG into. If omitted, the output will be printed to stdout.
    #[arg(long = "out", short = 'o')]
    out: Option<PathBuf>,
}

impl From<TemplateArgs> for TemplateOptions {
    fn from(value: TemplateArgs) -> Self {
        let mut this = Self {
            width: value.width,
            palette: svg::NamedPalette::from(value.palette).into(),
            line_numbers: value.line_numbers.map(svg::LineNumbers::from),
            window_frame: value.window_frame,
            scroll: value.scroll.map(|max_height| {
                max_height.map_or_else(ScrollOptions::default, |max_height| ScrollOptions {
                    max_height,
                    ..ScrollOptions::default()
                })
            }),
            wrap: if value.no_wrap {
                None
            } else {
                Some(WrapOptions::HardBreakAt(value.hard_wrap))
            },
            additional_styles: value.additional_styles.unwrap_or_default(),
            ..Self::default()
        };

        if let Some(path) = value.embed_font {
            this = this.with_font_subsetting(FontSubsetter::new(&path).unwrap_or_else(|err| {
                panic!("Failed loading font from {}: {err}", path.display());
            }));
        } else if let Some(mut font_family) = value.font_family {
            font_family.push_str(", monospace");
            this.font_family = font_family;
        }
        this
    }
}

impl TemplateArgs {
    pub fn create_input(&self, command: String) -> UserInput {
        let input = UserInput::command(command);
        if self.no_inputs {
            input.hide()
        } else {
            input
        }
    }

    pub fn render(mut self, transcript: &Transcript) -> anyhow::Result<()> {
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
            TemplateOptions::from(self)
        };

        let template = if let Some(template_path) = template_path {
            if template_path.as_os_str() == "-" {
                return Self::render_data(out_path.as_deref(), transcript, &options);
            }
            let template = Self::load_template(&template_path)?;
            Template::custom(template, options)
        } else if pure_svg {
            Template::pure_svg(options)
        } else {
            Template::new(options)
        };

        if let Some(out_path) = out_path {
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
        options: &TemplateOptions,
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
