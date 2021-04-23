//! Snapshot testing tools.

use termcolor::{Color, ColorChoice, ColorSpec, NoColor, StandardStream, WriteColor};

use std::{
    fs::File,
    io::{self, BufReader, Write},
    ops,
    path::Path,
};

use crate::{
    utils::IndentingWriter, Interaction, MatchKind, ParseError, Parsed, ShellOptions, Transcript,
};

/// Test output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TestOutput {
    /// Do not output anything.
    Quiet,
    /// Output normal amount of details.
    Normal,
    /// Output more details.
    Verbose,
}

/// Testing configuration.
#[derive(Debug)]
pub struct TestConfig {
    shell_options: ShellOptions,
    match_kind: MatchKind,
    output: TestOutput,
    color_choice: ColorChoice,
}

impl TestConfig {
    /// Creates a new config.
    pub fn new<Ext>(shell_options: ShellOptions<Ext>) -> Self {
        Self {
            shell_options: shell_options.drop_extensions(),
            match_kind: MatchKind::TextOnly,
            output: TestOutput::Normal,
            color_choice: ColorChoice::Auto,
        }
    }

    /// Sets the matching kind applied.
    pub fn with_match_kind(&mut self, kind: MatchKind) -> &mut Self {
        self.match_kind = kind;
        self
    }

    /// Sets coloring of the output.
    pub fn with_color_choice(&mut self, color_choice: ColorChoice) -> &mut Self {
        self.color_choice = color_choice;
        self
    }

    /// Configures test output.
    pub fn with_output(&mut self, output: TestOutput) -> &mut Self {
        self.output = output;
        self
    }

    /// Tests the `transcript`.
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during reproducing the transcript or processing
    ///   its output.
    /// - Panics if there are mismatches between outputs in the original and reproduced
    ///   transcripts.
    pub fn test_transcript(&mut self, transcript: &Transcript<Parsed>) {
        self.test_transcript_for_stats(transcript)
            .unwrap_or_else(|err| panic!("{}", err))
            .assert_no_errors();
    }

    /// Tests the `transcript` and returns testing results.
    ///
    /// # Errors
    ///
    /// - Returns an error if an error occurs during reproducing the transcript or processing
    ///   its output.
    pub fn test_transcript_for_stats(
        &mut self,
        transcript: &Transcript<Parsed>,
    ) -> io::Result<TestStats> {
        let inputs = transcript
            .interactions()
            .iter()
            .map(|interaction| interaction.input().to_owned());
        let reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)?;

        if self.output == TestOutput::Quiet {
            let mut out = NoColor::new(io::sink());
            self.compare_transcripts(&mut out, &transcript, &reproduced)
        } else {
            let out = StandardStream::stdout(self.color_choice);
            let mut out = out.lock();
            self.compare_transcripts(&mut out, &transcript, &reproduced)
        }
    }

    fn compare_transcripts(
        &self,
        out: &mut impl WriteColor,
        parsed: &Transcript<Parsed>,
        reproduced: &Transcript,
    ) -> io::Result<TestStats> {
        let it = parsed
            .interactions()
            .iter()
            .zip(reproduced.interactions().iter().map(Interaction::output));

        let mut stats = TestStats::default();
        for (original, reproduced) in it {
            let (original_text, reproduced_text) = match self.match_kind {
                MatchKind::Precise => {
                    let reproduced_html = reproduced
                        .to_html()
                        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
                    (original.output().html(), reproduced_html)
                }
                MatchKind::TextOnly => {
                    let reproduced_plaintext = reproduced
                        .to_plaintext()
                        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
                    (original.output().plaintext(), reproduced_plaintext)
                }
            };

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
                Self::write_diff(out, original_text, &reproduced_text)?;
            } else if self.output == TestOutput::Verbose {
                out.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(244))))?;
                let mut out_with_indents = IndentingWriter::new(&mut *out, b"    ");
                writeln!(out_with_indents, "{}", original.output().plaintext())?;
                out.reset()?;
            }
        }

        stats.print(out)?;
        writeln!(out)?;

        Ok(stats)
    }

    #[cfg(feature = "pretty_assertions")]
    fn write_diff(out: &mut impl Write, original: &str, reproduced: &str) -> io::Result<()> {
        use pretty_assertions::Comparison;
        use std::fmt;

        // Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
        // to switch to `fmt::Display`.
        struct DebugStr<'a>(&'a str);

        impl fmt::Debug for DebugStr<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                // Align output with verbose term output. Since `Comparison` adds one space,
                // we need to add 3 spaces instead of 4.
                for line in self.0.lines() {
                    writeln!(formatter, "   {}", line)?;
                }
                Ok(())
            }
        }

        write!(
            out,
            "    {}",
            Comparison::new(&DebugStr(original), &DebugStr(reproduced))
        )
    }
}

/// Stats of a single snapshot test.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct TestStats {
    /// Number of successfully matched user inputs.
    pub passed: usize,
    /// Number of unmatched user inputs.
    pub errors: usize,
}

impl TestStats {
    /// Panics if these stats contain errors.
    #[allow(clippy::missing_panics_doc)]
    pub fn assert_no_errors(self) {
        assert_eq!(self.errors, 0, "There were test errors");
    }

    #[doc(hidden)]
    pub fn print(self, out: &mut impl WriteColor) -> io::Result<()> {
        write!(out, "passed: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(out, "{}", self.passed)?;
        out.reset()?;

        write!(out, ", errors: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        write!(out, "{}", self.errors)?;
        out.reset()
    }
}

impl ops::AddAssign for TestStats {
    fn add_assign(&mut self, rhs: Self) {
        self.passed += rhs.passed;
        self.errors += rhs.errors;
    }
}

#[doc(hidden)] // public for the sake of the `read_transcript` macro
pub fn _read_transcript(
    including_file: &str,
    name: &str,
) -> Result<Transcript<Parsed>, ParseError> {
    let snapshot_path = Path::new(including_file)
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No parent of current file"))?
        .join(format!("snapshots/{}.svg", name));
    let svg = BufReader::new(File::open(snapshot_path)?);
    Transcript::from_svg(svg)
}

/// Reads the transcript from a file.
///
/// FIXME: more details
#[macro_export]
macro_rules! read_transcript {
    ($name:tt) => {
        $crate::test::_read_transcript(file!(), $name)
    };
}
