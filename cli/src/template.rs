//! Templating-related command-line args.

use structopt::StructOpt;

use std::{fs::File, io, mem, path::PathBuf};

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
        if let Some(out_path) = mem::take(&mut self.out) {
            let out = File::create(out_path)?;
            Template::new(self.into()).render(transcript, out)?;
        } else {
            Template::new(self.into()).render(transcript, io::stdout())?;
        }
        Ok(())
    }
}
