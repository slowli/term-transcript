//! Templating-related command-line args.

use anyhow::Context;
use handlebars::Template as HandlebarsTemplate;
use structopt::StructOpt;

use std::{
    fs::{self, File},
    io, mem,
    path::PathBuf,
};

use term_transcript::{
    svg::{NamedPalette, ScrollOptions, Template, TemplateOptions, WrapOptions},
    Transcript,
};

#[derive(Debug, StructOpt)]
pub(crate) struct TemplateArgs {
    /// Color palette to use.
    #[structopt(
        long,
        short = "p",
        default_value = "gjm8",
        possible_values = &["gjm8", "ubuntu", "xterm", "dracula", "powershell"]
    )]
    palette: NamedPalette,
    /// Adds a window frame around the rendered console.
    #[structopt(long = "window", short = "w")]
    window_frame: bool,
    /// Enables scrolling animation, but only if the snapshot height exceeds a threshold
    /// corresponding to ~19 lines.
    #[structopt(long)]
    scroll: bool,
    /// Disable text wrapping (by default, text is hard-wrapped at 80 chars). Line overflows
    /// will be hidden.
    #[structopt(long = "no-wrap")]
    no_wrap: bool,
    /// Path to a custom Handlebars template to use. `-` means not use a template at all,
    /// and instead output JSON data that would be fed to a template.
    #[structopt(long = "tpl")]
    template_path: Option<PathBuf>,
    /// File to save the rendered SVG into. If omitted, the output will be printed to stdout.
    #[structopt(long = "out", short = "o")]
    out: Option<PathBuf>,
}

impl From<TemplateArgs> for TemplateOptions {
    fn from(value: TemplateArgs) -> Self {
        Self {
            palette: value.palette.into(),
            window_frame: value.window_frame,
            scroll: if value.scroll {
                Some(ScrollOptions::default())
            } else {
                None
            },
            wrap: if value.no_wrap {
                None
            } else {
                Some(WrapOptions::default())
            },
            ..Self::default()
        }
    }
}

impl TemplateArgs {
    pub fn render(mut self, transcript: &Transcript) -> anyhow::Result<()> {
        let out_path = mem::take(&mut self.out);
        let template_path = mem::take(&mut self.template_path);
        let options = TemplateOptions::from(self);
        let template = if let Some(template_path) = template_path {
            if template_path.as_os_str() == "-" {
                let data = options
                    .render_data(transcript)
                    .context("cannot render data for Handlebars template")?;
                if let Some(out_path) = out_path {
                    let out = File::create(&out_path).with_context(|| {
                        format!(
                            "cannot create output file `{}`",
                            out_path.as_os_str().to_string_lossy()
                        )
                    })?;
                    serde_json::to_writer(out, &data).with_context(|| {
                        format!(
                            "cannot write Handlebars data to `{}`",
                            out_path.as_os_str().to_string_lossy()
                        )
                    })?;
                } else {
                    serde_json::to_writer(io::stdout(), &data)
                        .context("cannot write Handlebars data to stdout")?;
                }
                return Ok(());
            }

            let template_string = fs::read_to_string(&template_path).with_context(|| {
                format!(
                    "cannot read Handlebars template from `{}`",
                    template_path.as_os_str().to_string_lossy()
                )
            })?;
            let template = HandlebarsTemplate::compile(&template_string).with_context(|| {
                format!(
                    "cannot compile Handlebars template from `{}`",
                    template_path.as_os_str().to_string_lossy()
                )
            })?;
            Template::custom(template, options)
        } else {
            Template::new(options)
        };

        if let Some(out_path) = out_path {
            let out = File::create(&out_path).with_context(|| {
                format!(
                    "cannot create output file `{}`",
                    out_path.as_os_str().to_string_lossy()
                )
            })?;
            template.render(transcript, out).with_context(|| {
                format!(
                    "cannot render template to `{}`",
                    out_path.as_os_str().to_string_lossy()
                )
            })?;
        } else {
            template
                .render(transcript, io::stdout())
                .context("cannot render template to stdout")?;
        }
        Ok(())
    }
}
