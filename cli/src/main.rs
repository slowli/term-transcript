//! CLI for the `term-svg` crate.

use structopt::StructOpt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use std::{
    ffi::OsString,
    fs::File,
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{self, Command},
    str::FromStr,
};

use term_svg::{
    svg::{Template, TemplateOptions},
    test::{TestConfig, TestOutput, TestStats},
    MatchKind, ShellOptions, Transcript, UserInput,
};

/// CLI for capturing and snapshot-testing terminal output.
#[derive(Debug, StructOpt)]
enum Args {
    /// Captures output from stdin and renders it to SVG, then prints to stdout.
    Capture {
        /// Command to record as user input.
        command: String,
        // TODO: customize palette etc.
    },

    /// Executes one or more commands in a shell and renders the captured output to SVG,
    /// then prints to stdout.
    Exec {
        /// Shell command without args (they are supplied separately). If omitted,
        /// will be set to the default OS shell (`sh` for *NIX, `cmd` for Windows).
        #[structopt(long, short = "s")]
        shell: Option<OsString>,
        /// Arguments to supply to the shell command.
        #[structopt(name = "args", long, short = "a")]
        shell_args: Vec<OsString>,
        /// Inputs to supply to the shell.
        inputs: Vec<String>,
    },

    /// Tests previously captured SVG snapshots.
    Test {
        /// Shell command without args (they are supplied separately). If omitted,
        /// will be set to the default OS shell (`sh` for *NIX, `cmd` for Windows).
        #[structopt(long, short = "s")]
        shell: Option<OsString>,
        /// Arguments to supply to the shell command.
        #[structopt(name = "args", long, short = "a")]
        shell_args: Vec<OsString>,
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

impl Args {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Capture { command } => {
                let mut transcript = Transcript::new();
                let mut term_output = vec![];
                io::stdin().read_to_end(&mut term_output)?;
                transcript.add_interaction(UserInput::command(command), term_output);

                Template::new(TemplateOptions::default()).render(&transcript, io::stdout())?;
            }

            Self::Exec {
                shell,
                shell_args,
                inputs,
            } => {
                let inputs = inputs.into_iter().map(UserInput::command);
                let mut options = Self::shell_options(shell, shell_args);
                let transcript = Transcript::from_inputs(&mut options, inputs)?;

                Template::new(TemplateOptions::default()).render(&transcript, io::stdout())?;
            }

            Self::Test {
                shell,
                shell_args,
                svg_paths,
                precise,
                verbose,
                color,
            } => {
                let options = Self::shell_options(shell, shell_args);
                let mut test_config = TestConfig::new(options);
                test_config
                    .with_output(if verbose {
                        TestOutput::Verbose
                    } else {
                        TestOutput::Normal
                    })
                    .with_match_kind(if precise {
                        MatchKind::Precise
                    } else {
                        MatchKind::TextOnly
                    })
                    .with_color_choice(color.into());

                let mut totals = FullTestStats::default();
                let out = StandardStream::stdout(color.into());

                for svg_path in &svg_paths {
                    Self::report_test_start(&out, svg_path)?;
                    match Self::process_file(svg_path, &mut test_config) {
                        Ok(stats) => {
                            totals.base += stats;
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

    fn shell_options(shell: Option<OsString>, shell_args: Vec<OsString>) -> ShellOptions {
        if let Some(shell) = shell {
            let mut command = Command::new(shell);
            command.args(shell_args);
            command.into()
        } else {
            ShellOptions::default()
        }
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
    base: TestStats,
    failures: usize,
}

impl FullTestStats {
    fn print(self, out: &mut impl WriteColor) -> io::Result<()> {
        self.base.print(out)?;

        write!(out, ", failures: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
        write!(out, "{}", self.failures)?;
        out.reset()
    }

    fn is_successful(self) -> bool {
        self.base.errors == 0 && self.failures == 0
    }
}

fn main() -> anyhow::Result<()> {
    Args::from_args().run()
}
