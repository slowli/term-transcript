//! CLI for the `term-transcript` crate.

use anyhow::Context;
use clap::AppSettings;
use structopt::StructOpt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use std::{
    ffi::OsString,
    fs::File,
    io::{self, BufReader, Read, Write},
    mem,
    path::{Path, PathBuf},
    process::{self, Command},
    str::FromStr,
    time::Duration,
};

#[cfg(feature = "portable-pty")]
use term_transcript::PtyCommand;
use term_transcript::{
    svg::{NamedPalette, ScrollOptions, Template, TemplateOptions, WrapOptions},
    test::{MatchKind, TestConfig, TestOutputConfig, TestStats},
    ShellOptions, Transcript, UserInput,
};

/// CLI for capturing and snapshot-testing terminal output.
#[derive(Debug, StructOpt)]
#[structopt(global_setting = AppSettings::ColoredHelp)]
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

#[derive(Debug, StructOpt)]
struct ShellArgs {
    /// Execute shell in a pseudo-terminal (PTY), rather than connecting to it via pipes.
    #[cfg(feature = "portable-pty")]
    #[structopt(long)]
    pty: bool,
    /// Shell command without args (they are supplied separately). If omitted,
    /// will be set to the default OS shell (`sh` for *NIX, `cmd` for Windows).
    #[structopt(long, short = "s")]
    shell: Option<OsString>,
    /// Arguments to supply to the shell command.
    #[structopt(name = "args", long, short = "a")]
    shell_args: Vec<OsString>,
    /// Timeout for I/O operations in milliseconds.
    #[structopt(
        name = "io-timeout",
        long,
        short = "T",
        value_name = "millis",
        default_value = "1000"
    )]
    io_timeout: u64,
}

impl ShellArgs {
    fn into_std_options(self) -> ShellOptions {
        let options = if let Some(shell) = self.shell {
            let mut command = Command::new(shell);
            command.args(self.shell_args);
            command.into()
        } else {
            ShellOptions::default()
        };
        options.with_io_timeout(Duration::from_millis(self.io_timeout))
    }

    #[cfg(feature = "portable-pty")]
    fn into_pty_options(self) -> ShellOptions<PtyCommand> {
        let command = if let Some(shell) = self.shell {
            let mut command = PtyCommand::new(shell);
            for arg in self.shell_args {
                command.arg(arg);
            }
            command
        } else {
            PtyCommand::default()
        };
        ShellOptions::new(command).with_io_timeout(Duration::from_millis(self.io_timeout))
    }

    #[cfg(feature = "portable-pty")]
    fn create_transcript(
        self,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Transcript> {
        if self.pty {
            let mut options = self.into_pty_options();
            Transcript::from_inputs(&mut options, inputs)
        } else {
            let mut options = self.into_std_options();
            Transcript::from_inputs(&mut options, inputs)
        }
    }

    #[cfg(not(feature = "portable-pty"))]
    fn create_transcript(
        self,
        inputs: impl IntoIterator<Item = UserInput>,
    ) -> io::Result<Transcript> {
        let mut options = self.into_std_options();
        Transcript::from_inputs(&mut options, inputs)
    }
}

#[derive(Debug, StructOpt)]
struct TemplateArgs {
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
    fn render(mut self, transcript: &Transcript) -> anyhow::Result<()> {
        if let Some(out_path) = mem::take(&mut self.out) {
            let out = File::create(out_path)?;
            Template::new(self.into()).render(&transcript, out)?;
        } else {
            Template::new(self.into()).render(&transcript, io::stdout())?;
        }
        Ok(())
    }
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
