//! CLI for the `term-transcript` crate.

use anyhow::Context;
use structopt::StructOpt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use std::{
    ffi::OsString,
    fs::File,
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{self, Command},
    str::FromStr,
    time::Duration,
};

use term_transcript::{
    svg::{NamedPalette, Template, TemplateOptions},
    test::{MatchKind, TestConfig, TestOutputConfig, TestStats},
    ShellOptions, Transcript, UserInput,
};

/// CLI for capturing and snapshot-testing terminal output.
#[derive(Debug, StructOpt)]
enum Args {
    /// Captures output from stdin and renders it to SVG, then prints to stdout.
    Capture {
        /// Command to record as user input.
        command: String,
        #[structopt(flatten)]
        template: TemplateArgs,
    },

    /// Executes one or more commands in a shell and renders the captured output to SVG,
    /// then prints to stdout.
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
        /// Controls coloring of the output. One of `always`, `ansi`, `never` or `auto`.
        #[structopt(long, short = "c", default_value = "auto")]
        color: ColorPreference,
    },
}

#[derive(Debug, StructOpt)]
struct ShellArgs {
    /// Shell command without args (they are supplied separately). If omitted,
    /// will be set to the default OS shell (`sh` for *NIX, `cmd` for Windows).
    #[structopt(long, short = "s")]
    shell: Option<OsString>,
    /// Arguments to supply to the shell command.
    #[structopt(name = "args", long, short = "a")]
    shell_args: Vec<OsString>,
    /// Timeout for I/O operations in milliseconds.
    #[structopt(name = "io-timeout", long, short = "T", default_value = "1000")]
    io_timeout: u64,
}

impl ShellArgs {
    fn into_options(self) -> ShellOptions {
        let options = if let Some(shell) = self.shell {
            let mut command = Command::new(shell);
            command.args(self.shell_args);
            command.into()
        } else {
            ShellOptions::default()
        };
        options.with_io_timeout(Duration::from_millis(self.io_timeout))
    }
}

#[derive(Debug, StructOpt)]
struct TemplateArgs {
    /// Color palette to use.
    #[structopt(long, short = "p", default_value = "gjm8")]
    palette: NamedPalette,
    /// Adds a window frame around the rendered console.
    #[structopt(long = "window", short = "w")]
    window_frame: bool,
}

impl From<TemplateArgs> for TemplateOptions {
    fn from(value: TemplateArgs) -> Self {
        Self {
            palette: value.palette.into(),
            window_frame: value.window_frame,
            ..Self::default()
        }
    }
}

impl Args {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Capture { command, template } => {
                let mut transcript = Transcript::new();
                let mut term_output = vec![];
                io::stdin().read_to_end(&mut term_output)?;

                let term_output = String::from_utf8(term_output)
                    .map_err(|err| err.utf8_error())
                    .with_context(|| "Failed to convert terminal output to UTF-8")?;
                transcript.add_interaction(UserInput::command(command), term_output);

                Template::new(template.into()).render(&transcript, io::stdout())?;
            }

            Self::Exec {
                shell,
                inputs,
                template,
            } => {
                let inputs = inputs.into_iter().map(UserInput::command);
                let mut options = shell.into_options();
                let transcript = Transcript::from_inputs(&mut options, inputs)?;

                Template::new(template.into()).render(&transcript, io::stdout())?;
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
                let options = shell.into_options();

                let mut test_config = TestConfig::new(options);
                test_config
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
        }
        Ok(())
    }

    fn process_file(svg_path: &Path, test_config: &mut TestConfig) -> anyhow::Result<TestStats> {
        let svg = BufReader::new(File::open(svg_path)?);
        let transcript = Transcript::from_svg(svg)?;
        test_config
            .test_transcript_for_stats(&transcript)
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
        writeln!(out, ": {}", err)?;
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
