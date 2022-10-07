//! CLI for the `term-transcript` crate.

use anyhow::Context;
use clap::AppSettings;
use structopt::StructOpt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    process,
    str::FromStr,
};

use term_transcript::{
    test::{MatchKind, TestConfig, TestOutputConfig, TestStats},
    Transcript, UserInput,
};

mod shell;
mod template;

use crate::{shell::ShellArgs, template::TemplateArgs};

/// CLI for capturing and snapshot-testing terminal output.
#[derive(Debug, StructOpt)]
#[structopt(global_setting = AppSettings::ColoredHelp)]
enum Args {
    /// Captures output from stdin and renders it to SVG.
    Capture {
        /// Command to record as user input.
        command: String,
        #[structopt(flatten)]
        template: TemplateArgs,
    },

    /// Executes one or more commands in a shell and renders the captured output to SVG.
    Exec {
        #[structopt(flatten)]
        shell: ShellArgs,
        /// Inputs to supply to the shell.
        inputs: Vec<String>,
        #[structopt(flatten)]
        template: TemplateArgs,
    },

    /// Tests previously captured SVG snapshots.
    Test {
        #[structopt(flatten)]
        shell: ShellArgs,
        /// Paths to the SVG file(s) to test.
        #[structopt(name = "svg")]
        svg_paths: Vec<PathBuf>,
        /// Prints terminal output for passed user inputs.
        #[structopt(long, short = "v")]
        verbose: bool,
        /// Matches coloring of the terminal output, rather than matching only text.
        #[structopt(long, short = "p")]
        precise: bool,
        /// Controls coloring of the output.
        #[structopt(
            long,
            short = "c",
            default_value = "auto",
            possible_values = &["always", "ansi", "never", "auto"],
            env
        )]
        color: ColorPreference,
    },

    /// Prints a previously saved SVG file to stdout with the captured coloring (unless
    /// the coloring of the output is switched off).
    Print {
        /// Path to the SVG file to output.
        #[structopt(name = "svg")]
        svg_path: PathBuf,
        /// Controls coloring of the output.
        #[structopt(
            long,
            short = "c",
            default_value = "auto",
            possible_values = &["always", "ansi", "never", "auto"],
            env
        )]
        color: ColorPreference,
    },
}

impl Args {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Capture { command, template } => {
                let mut transcript = Transcript::new();
                let mut term_output = vec![];
                io::stdin().read_to_end(&mut term_output)?;

                let mut term_output = String::from_utf8(term_output)
                    .map_err(|err| err.utf8_error())
                    .with_context(|| "Failed to convert terminal output to UTF-8")?;
                // Trim the ending newline.
                if term_output.ends_with('\n') {
                    term_output.pop();
                }

                transcript.add_interaction(UserInput::command(command), term_output);
                template.render(&transcript)?;
            }

            Self::Exec {
                shell,
                inputs,
                template,
            } => {
                let inputs = inputs.into_iter().map(UserInput::command);
                let transcript = shell.create_transcript(inputs)?;
                template.render(&transcript)?;
            }

            Self::Test {
                shell,
                svg_paths,
                precise,
                verbose,
                color,
            } => {
                let match_kind = if precise {
                    MatchKind::Precise
                } else {
                    MatchKind::TextOnly
                };
                let options = shell.into_std_options();

                let mut test_config = TestConfig::new(options)
                    .with_output(if verbose {
                        TestOutputConfig::Verbose
                    } else {
                        TestOutputConfig::Normal
                    })
                    .with_match_kind(match_kind)
                    .with_color_choice(color.into());

                let mut totals = FullTestStats::default();
                let out = StandardStream::stdout(color.into());

                for svg_path in &svg_paths {
                    Self::report_test_start(&out, svg_path)?;
                    match Self::process_file(svg_path, &mut test_config) {
                        Ok(stats) => {
                            totals.passed += stats.passed(match_kind);
                            totals.errors += stats.errors(match_kind);
                        }
                        Err(err) => {
                            Self::report_failure(&out, svg_path, err)?;
                            totals.failures += 1;
                        }
                    }
                }

                Self::report_totals(&out, totals)?;

                if !totals.is_successful() {
                    process::exit(1);
                }
            }

            Self::Print { svg_path, color } => Self::print_file(&svg_path, color)?,
        }
        Ok(())
    }

    fn process_file(svg_path: &Path, test_config: &mut TestConfig) -> anyhow::Result<TestStats> {
        let svg = BufReader::new(File::open(svg_path)?);
        let transcript = Transcript::from_svg(svg)?;
        test_config
            .test_transcript_for_stats(&transcript)
            .map(|(stats, _)| stats)
            .map_err(From::from)
    }

    fn report_test_start(out: &StandardStream, svg_path: &Path) -> io::Result<()> {
        let mut out = out.lock();
        write!(out, "Testing file ")?;
        out.set_color(ColorSpec::new().set_intense(true).set_underline(true))?;
        write!(out, "{}", svg_path.to_string_lossy())?;
        out.reset()?;
        writeln!(out, "...")
    }

    fn report_failure(out: &StandardStream, svg_path: &Path, err: anyhow::Error) -> io::Result<()> {
        let mut out = out.lock();
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        write!(out, "Error testing file ")?;
        out.set_color(
            ColorSpec::new()
                .set_reset(false)
                .set_intense(true)
                .set_underline(true),
        )?;
        write!(out, "{}", svg_path.to_string_lossy())?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        writeln!(out, ": {err}")?;
        out.reset()
    }

    fn report_totals(out: &StandardStream, totals: FullTestStats) -> io::Result<()> {
        let mut out = out.lock();
        out.set_color(ColorSpec::new().set_intense(true))?;
        write!(out, "Totals: ")?;
        out.reset()?;
        totals.print(&mut out)?;
        writeln!(out)
    }

    fn print_file(svg_path: &Path, color: ColorPreference) -> anyhow::Result<()> {
        let svg = BufReader::new(File::open(svg_path)?);
        let transcript = Transcript::from_svg(svg)?;

        let color = ColorChoice::from(color);
        let out = StandardStream::stdout(color);
        let mut out = out.lock();

        for (i, interaction) in transcript.interactions().iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            out.set_color(ColorSpec::new().set_bold(true))?;
            writeln!(out, "----------  Input #{} ----------", i + 1)?;
            out.reset()?;

            let input = interaction.input();
            writeln!(out, "{} {}", input.prompt().unwrap_or("$"), input.as_ref())?;

            out.set_color(ColorSpec::new().set_bold(true))?;
            writeln!(out, "\n---------- Output #{} ----------", i + 1)?;
            out.reset()?;

            if color == ColorChoice::Never {
                writeln!(out, "{}", interaction.output().plaintext())?;
            } else {
                interaction.output().write_colorized(&mut out)?;
                out.reset()?;
                if !interaction.output().plaintext().ends_with('\n') {
                    writeln!(out)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum ColorPreference {
    Always,
    Ansi,
    Auto,
    Never,
}

impl FromStr for ColorPreference {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "always" => Self::Always,
            "ansi" => Self::Ansi,
            "auto" => Self::Auto,
            "never" => Self::Never,
            _ => return Err(anyhow::anyhow!("Unrecognized color preference")),
        })
    }
}

impl From<ColorPreference> for ColorChoice {
    fn from(value: ColorPreference) -> Self {
        match value {
            ColorPreference::Always => ColorChoice::Always,
            ColorPreference::Ansi => ColorChoice::AlwaysAnsi,
            ColorPreference::Auto => {
                if atty::is(atty::Stream::Stdout) {
                    ColorChoice::Auto
                } else {
                    ColorChoice::Never
                }
            }
            ColorPreference::Never => ColorChoice::Never,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FullTestStats {
    passed: usize,
    errors: usize,
    failures: usize,
}

impl FullTestStats {
    fn print(self, out: &mut impl WriteColor) -> io::Result<()> {
        write!(out, "passed: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(out, "{}", self.passed)?;
        out.reset()?;

        write!(out, ", errors: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        write!(out, "{}", self.errors)?;
        out.reset()?;

        write!(out, ", failures: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
        write!(out, "{}", self.failures)?;
        out.reset()
    }

    fn is_successful(self) -> bool {
        self.errors == 0 && self.failures == 0
    }
}

fn main() -> anyhow::Result<()> {
    Args::from_args().run()
}
