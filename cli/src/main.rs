//! CLI for the `term-transcript` crate.

use std::{
    fmt,
    fs::File,
    io::{self, BufReader, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process,
    str::FromStr,
};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use term_transcript::{
    test::{MatchKind, TestConfig, TestOutputConfig, TestStats},
    traits::SpawnShell,
    Transcript, UserInput,
};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

mod shell;
mod template;
#[cfg(test)]
mod tests;

use crate::{shell::ShellArgs, template::TemplateArgs};

/// CLI for capturing and snapshot-testing terminal output.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Captures output from stdin and renders it to SVG.
    Capture {
        /// Command to record as user input.
        command: String,
        #[command(flatten)]
        template: TemplateArgs,
    },

    /// Executes one or more commands in a shell and renders the captured output to SVG.
    Exec {
        #[command(flatten)]
        shell: ShellArgs,
        /// Inputs to supply to the shell.
        inputs: Vec<String>,
        #[command(flatten)]
        template: TemplateArgs,
    },

    /// Tests previously captured SVG snapshots.
    Test {
        #[command(flatten)]
        shell: ShellArgs,
        /// Paths to the SVG file(s) to test.
        #[arg(name = "svg")]
        svg_paths: Vec<PathBuf>,
        /// Prints terminal output for passed user inputs.
        #[arg(long, short = 'v')]
        verbose: bool,
        /// Matches coloring of the terminal output, rather than matching only text.
        #[arg(long, short = 'p')]
        precise: bool,
        /// Controls coloring of the output.
        #[arg(long, short = 'c', default_value = "auto", value_enum, env)]
        color: ColorPreference,
    },

    /// Prints a previously saved SVG file to stdout with the captured coloring (unless
    /// the coloring of the output is switched off).
    Print {
        /// Path to the SVG file to output. If set to `-`, the SVG will be read from stdin.
        #[arg(name = "svg")]
        svg_path: PathBuf,
        /// Controls coloring of the output.
        #[arg(long, short = 'c', default_value = "auto", value_enum, env)]
        color: ColorPreference,
    },
}

impl Command {
    fn run(self) -> anyhow::Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(?self, "running command");

        match self {
            Self::Capture { command, template } => {
                #[cfg(feature = "tracing")]
                let _entered = tracing::info_span!("capture").entered();

                let no_inputs = template.no_inputs;
                let template = template.build()?;

                let mut transcript = Transcript::new();
                let mut term_output = vec![];
                #[cfg(feature = "tracing")]
                tracing::info!("capturing stdin");
                io::stdin().read_to_end(&mut term_output)?;
                #[cfg(feature = "tracing")]
                tracing::info!(output.len = term_output.len(), "captured stdin");

                let mut term_output = String::from_utf8(term_output)
                    .map_err(|err| err.utf8_error())
                    .with_context(|| "Failed to convert terminal output to UTF-8")?;
                // Trim the ending newline.
                if term_output.ends_with('\n') {
                    term_output.pop();
                }

                transcript.add_interaction(Self::create_input(command, no_inputs), term_output);
                #[cfg(feature = "tracing")]
                tracing::info!("rendering transcript");
                template.render(&transcript)?;
            }

            Self::Exec {
                shell,
                inputs,
                template,
            } => {
                #[cfg(feature = "tracing")]
                let _entered = tracing::info_span!("exec").entered();

                let no_inputs = template.no_inputs;
                let template = template.build()?;
                let inputs = inputs
                    .into_iter()
                    .map(|input| Self::create_input(input, no_inputs));
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
                #[cfg(feature = "tracing")]
                let _entered = tracing::info_span!("test").entered();

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
                            Self::report_failure(&out, svg_path, &err)?;
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

    fn create_input(command: String, no_inputs: bool) -> UserInput {
        let input = UserInput::command(command);
        if no_inputs {
            input.hide()
        } else {
            input
        }
    }

    fn process_file<Cmd: SpawnShell + fmt::Debug>(
        svg_path: &Path,
        test_config: &mut TestConfig<Cmd>,
    ) -> anyhow::Result<TestStats> {
        let svg = BufReader::new(File::open(svg_path)?);
        let transcript = Transcript::from_svg(svg)?;
        #[cfg(feature = "tracing")]
        tracing::info!(
            ?svg_path,
            transcript.len = transcript.interactions().len(),
            "parsed transcript"
        );

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

    fn report_failure(
        out: &StandardStream,
        svg_path: &Path,
        err: &anyhow::Error,
    ) -> io::Result<()> {
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

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn print_file(svg_path: &Path, color: ColorPreference) -> anyhow::Result<()> {
        let transcript = if svg_path.as_os_str() == "-" {
            let svg = BufReader::new(io::stdin());
            Transcript::from_svg(svg)?
        } else {
            let svg = BufReader::new(File::open(svg_path)?);
            Transcript::from_svg(svg)?
        };
        #[cfg(feature = "tracing")]
        tracing::info!(
            ?svg_path,
            transcript.len = transcript.interactions().len(),
            "parsed transcript"
        );

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

            if let Some(exit_status) = interaction.exit_status() {
                if !exit_status.is_success() {
                    out.set_color(ColorSpec::new().set_bold(true))?;
                    write!(out, "Exit status:")?;
                    out.reset()?;
                    write!(out, " {} ", exit_status.0)?;

                    out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
                    writeln!(out, "(failure)")?;
                    out.reset()?;
                }
            }

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

#[derive(Debug, Clone, Copy, ValueEnum)]
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
                if io::stdout().is_terminal() {
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

#[cfg(feature = "tracing")]
fn setup_tracing() {
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    FmtSubscriber::builder()
        .pretty()
        .with_writer(io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "tracing")]
    setup_tracing();

    Cli::parse().command.run()
}
