//! CLI for the `term-svg` crate.

use pretty_assertions::Comparison;
use structopt::StructOpt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use std::{
    ffi::OsString,
    fmt,
    fs::File,
    io::{self, BufReader, Read, Write},
    ops,
    path::{Path, PathBuf},
    process::{self, Command},
    str::FromStr,
    time::Duration,
};

use term_svg::{
    Interaction, Parsed, ShellOptions, SvgTemplate, SvgTemplateOptions, Transcript, UserInput,
};

/// CLI for capturing and snapshot-testing terminal output.
#[derive(Debug, StructOpt)]
enum Args {
    /// Captures output from stdin and renders it to SVG, which is output to stdout.
    Capture {
        /// Command to record as user input.
        command: String,
        // TODO: customize palette etc.
    },

    /// Executes one or more commands in a shell and renders the captured output to SVG,
    /// which is output to stdout.
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

    /// Tests SVG snapshots.
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
        /// Controls coloring of the output.
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

                SvgTemplate::new(SvgTemplateOptions::default())
                    .render(&transcript, io::stdout())?;
            }

            Self::Exec {
                shell,
                shell_args,
                inputs,
            } => {
                let inputs = inputs.into_iter().map(UserInput::command);
                let mut options = Self::shell_options(shell, shell_args);
                let transcript = Transcript::from_inputs(&mut options, inputs)?;

                SvgTemplate::new(SvgTemplateOptions::default())
                    .render(&transcript, io::stdout())?;
            }

            Self::Test {
                shell,
                shell_args,
                svg_paths,
                verbose,
                color,
            } => {
                let mut options = Self::shell_options(shell, shell_args);
                let mut totals = TestStats::default();
                let out = StandardStream::stdout(color.into());
                let mut out = out.lock();

                for svg_path in &svg_paths {
                    write!(out, "Testing file ")?;
                    out.set_color(ColorSpec::new().set_intense(true).set_underline(true))?;
                    write!(out, "{}", svg_path.to_string_lossy())?;
                    out.reset()?;
                    writeln!(out, "...")?;

                    match Self::process_file(&mut out, svg_path, &mut options, verbose) {
                        Ok(stats) => totals += stats,
                        Err(err) => {
                            Self::report_failure(&mut out, svg_path, err)?;
                            totals.failures += 1;
                        }
                    }
                }

                out.set_color(ColorSpec::new().set_intense(true))?;
                write!(out, "Totals: ")?;
                out.reset()?;

                totals.print(&mut out)?;
                writeln!(out)?;
                if !totals.is_successful() {
                    process::exit(1);
                }
            }
        }
        Ok(())
    }

    fn process_file(
        out: &mut impl WriteColor,
        svg_path: &Path,
        options: &mut ShellOptions,
        verbose: bool,
    ) -> anyhow::Result<TestStats> {
        let svg = BufReader::new(File::open(svg_path)?);
        let transcript = Transcript::from_svg(svg)?;
        Self::test_transcript(out, &transcript, options, verbose)
    }

    fn report_failure(
        out: &mut impl WriteColor,
        svg_path: &Path,
        err: anyhow::Error,
    ) -> io::Result<()> {
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

    fn shell_options(shell: Option<OsString>, shell_args: Vec<OsString>) -> ShellOptions {
        if let Some(shell) = shell {
            let mut command = Command::new(shell);
            command.args(shell_args);
            ShellOptions::new(command, Duration::from_secs(1))
        } else {
            ShellOptions::default()
        }
    }

    fn test_transcript(
        out: &mut impl WriteColor,
        transcript: &Transcript<Parsed>,
        options: &mut ShellOptions,
        verbose: bool,
    ) -> anyhow::Result<TestStats> {
        let inputs = transcript
            .interactions()
            .iter()
            .map(|interaction| interaction.input().to_owned());
        let reproduced = Transcript::from_inputs(options, inputs)?;

        let it = transcript
            .interactions()
            .iter()
            .zip(reproduced.interactions().iter().map(Interaction::output));

        let mut stats = TestStats::default();
        for (original, reproduced) in it {
            let original_text = original.output().plaintext();
            let reproduced_text = reproduced.to_plaintext()?;

            write!(out, "  ")?;
            out.set_color(ColorSpec::new().set_intense(true))?;
            write!(out, "[")?;

            if original_text == reproduced_text {
                stats.passed += 1;
                out.set_color(ColorSpec::new().set_reset(false).set_fg(Some(Color::Green)))?;
                write!(out, "+")?;
            } else {
                stats.errors += 1;
                out.set_color(ColorSpec::new().set_reset(false).set_fg(Some(Color::Red)))?;
                write!(out, "-")?;
            }

            out.set_color(ColorSpec::new().set_intense(true))?;
            write!(out, "]")?;
            out.reset()?;
            writeln!(out, " Input: {}", original.input().as_ref())?;

            if original_text != reproduced_text {
                write!(
                    out,
                    "    {}",
                    Comparison::new(&DebugStr(original_text), &DebugStr(&reproduced_text))
                )?;
            } else if verbose {
                out.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(244))))?;
                writeln!(
                    out,
                    "{}",
                    textwrap::indent(original_text, "    ").trim_end()
                )?;
                out.reset()?;
            }
        }
        Ok(stats)
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

// Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
// to switch to `fmt::Display`.
struct DebugStr<'a>(&'a str);

impl fmt::Debug for DebugStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Align output with verbose term output. Since `Comparison` adds one space,
        // we need to add 3 spaces instead of 4.
        formatter.write_str(&textwrap::indent(self.0, "   "))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TestStats {
    passed: usize,
    errors: usize,
    failures: usize,
}

impl TestStats {
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

impl ops::AddAssign for TestStats {
    fn add_assign(&mut self, rhs: Self) {
        self.passed += rhs.passed;
        self.errors += rhs.errors;
        self.failures += rhs.failures;
    }
}

fn main() -> anyhow::Result<()> {
    Args::from_args().run()
}
