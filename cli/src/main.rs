//! CLI for the `term-svg` crate.

use pretty_assertions::Comparison;
use structopt::StructOpt;

use std::{
    ffi::OsString,
    fmt,
    fs::File,
    io::{self, BufReader, Read},
    ops,
    path::{Path, PathBuf},
    process::{self, Command},
    time::Duration,
};

use term_svg::{
    Interaction, Parsed, ShellOptions, SvgTemplate, SvgTemplateOptions, Transcript, UserInput,
};

// TODO: use coloring for CLI output.

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
            } => {
                let mut options = Self::shell_options(shell, shell_args);
                let mut totals = TestStats::default();

                for svg_path in &svg_paths {
                    match Self::process_file(svg_path, &mut options, verbose) {
                        Ok(stats) => totals += stats,
                        Err(err) => {
                            eprintln!("Error testing file {}: {}", svg_path.to_string_lossy(), err);
                            totals.failures += 1;
                        }
                    }
                }

                print!("Totals: ");
                totals.print();
                println!();
                if !totals.is_successful() {
                    eprintln!("There were test failures");
                    process::exit(1);
                }
            }
        }
        Ok(())
    }

    fn process_file(
        svg_path: &Path,
        options: &mut ShellOptions,
        verbose: bool,
    ) -> anyhow::Result<TestStats> {
        println!("Testing file {}...", svg_path.to_string_lossy());
        let svg = BufReader::new(File::open(svg_path)?);
        let transcript = Transcript::from_svg(svg)?;
        Self::test_transcript(&transcript, options, verbose)
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
            let status = if original_text == reproduced_text {
                stats.passed += 1;
                '+'
            } else {
                stats.errors += 1;
                '-'
            };
            println!("  [{}] Input: {}", status, original.input().as_ref());

            if original_text != reproduced_text {
                print!(
                    "{}",
                    Comparison::new(&DebugStr(original_text), &DebugStr(&reproduced_text))
                );
            } else if verbose {
                println!("{}", textwrap::indent(original_text, "    ").trim_end());
            }
        }
        Ok(stats)
    }
}

// Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
// to switch to `fmt::Display`.
struct DebugStr<'a>(&'a str);

impl fmt::Debug for DebugStr<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    fn print(self) {
        print!(
            "passed: {}, errors: {}, failures: {}",
            self.passed, self.errors, self.failures
        );
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
