//! Snapshot testing tools for [`Transcript`]s.
//!
//! # Examples
//!
//! Simple scenario in which the tested transcript calls to one or more Cargo binaries / examples
//! by their original names.
//!
//! ```
//! use term_svg::{
//!     read_svg_snapshot, ShellOptions, Transcript,
//!     test::{MatchKind, TestConfig, TestOutputConfig},
//! };
//!
//! // Test configuration that can be shared across tests.
//! fn config() -> TestConfig {
//!     let shell_options = ShellOptions::default().with_cargo_path();
//!     let mut config = TestConfig::new(shell_options);
//!     config
//!         .with_match_kind(MatchKind::Precise)
//!         .with_output(TestOutputConfig::Verbose);
//!     config
//! }
//!
//! // Usage in tests:
//! fn test_basics() -> anyhow::Result<()> {
//!     let transcript = Transcript::from_svg(read_svg_snapshot!("basic")?)?;
//!     config().test_transcript(&transcript);
//!     Ok(())
//! }
//! ```

use termcolor::{Color, ColorChoice, ColorSpec, NoColor, StandardStream, WriteColor};

use std::{
    fs::File,
    io::{self, BufReader, Write},
    path::Path,
};

use crate::{utils::IndentingWriter, Interaction, Parsed, ShellOptions, Transcript};

/// Configuration of output produced during testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TestOutputConfig {
    /// Do not output anything.
    Quiet,
    /// Output normal amount of details.
    Normal,
    /// Output more details.
    Verbose,
}

impl Default for TestOutputConfig {
    fn default() -> Self {
        Self::Normal
    }
}

/// Testing configuration.
#[derive(Debug)]
pub struct TestConfig {
    shell_options: ShellOptions,
    match_kind: MatchKind,
    output: TestOutputConfig,
    color_choice: ColorChoice,
}

impl TestConfig {
    /// Creates a new config.
    pub fn new<Ext>(shell_options: ShellOptions<Ext>) -> Self {
        Self {
            shell_options: shell_options.drop_extensions(),
            match_kind: MatchKind::TextOnly,
            output: TestOutputConfig::Normal,
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
    pub fn with_output(&mut self, output: TestOutputConfig) -> &mut Self {
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
            .assert_no_errors(self.match_kind);
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

        if self.output == TestOutputConfig::Quiet {
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

        let mut stats = TestStats {
            matches: Vec::with_capacity(parsed.interactions().len()),
        };
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
                stats.matches.push(Some(self.match_kind));
                out.set_color(ColorSpec::new().set_reset(false).set_fg(Some(Color::Green)))?;
                write!(out, "+")?;
            } else {
                // TODO: there can still be a text match if we check by HTML.
                stats.matches.push(None);
                out.set_color(ColorSpec::new().set_reset(false).set_fg(Some(Color::Red)))?;
                write!(out, "-")?;
            }

            out.set_color(ColorSpec::new().set_intense(true))?;
            write!(out, "]")?;
            out.reset()?;
            writeln!(out, " Input: {}", original.input().as_ref())?;

            if original_text != reproduced_text {
                Self::write_diff(out, original_text, &reproduced_text)?;
            } else if self.output == TestOutputConfig::Verbose {
                out.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(244))))?;
                let mut out_with_indents = IndentingWriter::new(&mut *out, b"    ");
                writeln!(out_with_indents, "{}", original.output().plaintext())?;
                out.reset()?;
            }
        }

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

    #[cfg(not(feature = "pretty_assertions"))]
    fn write_diff(out: &mut impl Write, original: &str, reproduced: &str) -> io::Result<()> {
        writeln!(out, "  Original:")?;
        for line in original.lines() {
            writeln!(out, "    {}", line)?;
        }
        writeln!(out, "  Reproduced:")?;
        for line in reproduced.lines() {
            writeln!(out, "    {}", line)?;
        }
        Ok(())
    }
}

/// Kind of terminal output matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum MatchKind {
    /// Relaxed matching: compare only output text, but not coloring.
    TextOnly,
    /// Precise matching: compare output together with colors.
    Precise,
}

/// Stats of a single snapshot test.
#[derive(Debug, Clone)]
pub struct TestStats {
    // Match kind per each user input.
    matches: Vec<Option<MatchKind>>,
}

impl TestStats {
    /// Returns the number of successfully matched user inputs with at least the specified
    /// `match_level`.
    pub fn passed(&self, match_level: MatchKind) -> usize {
        self.matches
            .iter()
            .filter(|&&kind| kind >= Some(match_level))
            .count()
    }

    /// Returns the number of user inputs that do not match with at least the specified
    /// `match_level`.
    pub fn errors(&self, match_level: MatchKind) -> usize {
        self.matches.len() - self.passed(match_level)
    }

    /// Returns match kinds per each user input of the tested [`Transcript`]. `None` values
    /// mean no match.
    pub fn matches(&self) -> &[Option<MatchKind>] {
        &self.matches
    }

    /// Panics if these stats contain errors.
    #[allow(clippy::missing_panics_doc)]
    pub fn assert_no_errors(&self, match_level: MatchKind) {
        assert_eq!(self.errors(match_level), 0, "There were test errors");
    }

    fn print_summary(&self, out: &mut impl WriteColor, match_level: MatchKind) -> io::Result<()> {
        write!(out, "passed: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(out, "{}", self.passed(match_level))?;
        out.reset()?;

        write!(out, ", errors: ")?;
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        write!(out, "{}", self.errors(match_level))?;
        out.reset()
    }
}

#[doc(hidden)] // public for the sake of the `read_transcript` macro
pub fn _read_svg_snapshot(including_file: &str, name: &str) -> io::Result<BufReader<File>> {
    let snapshot_path = Path::new(including_file)
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No parent of current file"))?
        .join(format!("snapshots/{}.svg", name));
    Ok(BufReader::new(File::open(snapshot_path)?))
}

/// Reads an SVG transcript snapshot from a file. Returns a [buffered reader] for the [`File`].
///
/// Similarly to [`insta`], the transcript is searched in the `snapshot` directory adjacent to
/// the file invoking the macro. The `.svg` extension is automatically added to the provided name.
///
/// # Errors
///
/// - Returns I/O errors that can occur when reading the file (e.g., if the file does not exist).
///
/// [buffered reader]: BufReader
/// [`insta`]: https://insta.rs/
///
/// # Examples
///
/// ```
/// use term_svg::{read_svg_snapshot, Transcript};
/// # use std::io;
///
/// # fn unused() -> anyhow::Result<()> {
/// // Will read from `snapshots/my-test.svg`
/// let transcript = Transcript::from_svg(read_svg_snapshot!("my-test")?)?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! read_svg_snapshot {
    ($name:tt) => {
        $crate::test::_read_svg_snapshot(file!(), $name)
    };
}
