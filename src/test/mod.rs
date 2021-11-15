//! Snapshot testing tools for [`Transcript`]s.
//!
//! # Examples
//!
//! Simple scenario in which the tested transcript calls to one or more Cargo binaries / examples
//! by their original names.
//!
//! ```
//! use term_transcript::{
//!     ShellOptions, Transcript,
//!     test::{MatchKind, TestConfig, TestOutputConfig},
//! };
//! use std::io;
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
//! fn read_svg_snapshot() -> io::Result<impl io::BufRead> {
//!     // reads the snapshot, e.g. from a file
//! #   Ok(io::Cursor::new(vec![]))
//! }
//!
//! // Usage in tests:
//! fn test_basics() -> anyhow::Result<()> {
//!     let transcript = Transcript::from_svg(read_svg_snapshot()?)?;
//!     config().test_transcript(&transcript);
//!     Ok(())
//! }
//! ```

use termcolor::{Color, ColorChoice, ColorSpec, NoColor, StandardStream, WriteColor};

use std::{
    io::{self, Write},
    process::Command,
    str,
};

use crate::{traits::SpawnShell, Interaction, ShellOptions, TermError, Transcript};

mod color_diff;
mod parser;

use self::color_diff::ColorSpan;
pub use self::parser::{ParseError, Parsed};
use crate::test::color_diff::ColorDiff;

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
///
/// # Examples
///
/// See the [module docs](crate::test) for the examples of usage.
#[derive(Debug)]
pub struct TestConfig<Cmd = Command> {
    shell_options: ShellOptions<Cmd>,
    match_kind: MatchKind,
    output: TestOutputConfig,
    color_choice: ColorChoice,
}

impl<Cmd: SpawnShell> TestConfig<Cmd> {
    /// Creates a new config.
    pub fn new(shell_options: ShellOptions<Cmd>) -> Self {
        Self {
            shell_options,
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
        if self.output == TestOutputConfig::Quiet {
            let mut out = NoColor::new(io::sink());
            self.test_transcript_inner(&mut out, transcript)
        } else {
            let out = StandardStream::stdout(self.color_choice);
            let mut out = out.lock();
            self.test_transcript_inner(&mut out, transcript)
        }
    }

    fn test_transcript_inner(
        &mut self,
        out: &mut impl WriteColor,
        transcript: &Transcript<Parsed>,
    ) -> io::Result<TestStats> {
        let inputs = transcript
            .interactions()
            .iter()
            .map(|interaction| interaction.input().clone());
        let reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)?;

        self.compare_transcripts(out, transcript, &reproduced)
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
            write!(out, "  ")?;
            out.set_color(ColorSpec::new().set_intense(true))?;
            write!(out, "[")?;

            // First, process text only.
            let original_text = original.output().plaintext();
            let reproduced_text = reproduced
                .to_plaintext()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            let mut actual_match = if original_text == reproduced_text {
                Some(MatchKind::TextOnly)
            } else {
                None
            };

            // If we do precise matching, check it as well.
            let color_diff = if self.match_kind == MatchKind::Precise && actual_match.is_some() {
                let original_spans =
                    ColorSpan::parse(original.output().ansi_text()).map_err(|err| match err {
                        TermError::Io(err) => err,
                        other => io::Error::new(io::ErrorKind::InvalidInput, other),
                    })?;
                let reproduced_spans =
                    ColorSpan::parse(reproduced.as_ref()).map_err(|err| match err {
                        TermError::Io(err) => err,
                        other => io::Error::new(io::ErrorKind::InvalidInput, other),
                    })?;

                let diff = ColorDiff::new(&original_spans, &reproduced_spans);
                if diff.is_empty() {
                    actual_match = Some(MatchKind::Precise);
                    None
                } else {
                    Some(diff)
                }
            } else {
                None
            };

            stats.matches.push(actual_match);
            if actual_match >= Some(self.match_kind) {
                out.set_color(ColorSpec::new().set_reset(false).set_fg(Some(Color::Green)))?;
                write!(out, "+")?;
            } else {
                out.set_color(ColorSpec::new().set_reset(false).set_fg(Some(Color::Red)))?;
                write!(out, "-")?;
            }
            out.set_color(ColorSpec::new().set_intense(true))?;
            write!(out, "]")?;
            out.reset()?;
            writeln!(out, " Input: {}", original.input().as_ref())?;

            if let Some(diff) = color_diff {
                if out.supports_color() {
                    diff.highlight_on_text(out, original_text)?;
                    writeln!(out)?;
                }
                // TODO: highlight with `^^^`s if color is not supported?
                diff.write_as_table(out)?;
            } else if actual_match.is_none() {
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

/// Stats of a single snapshot test output by [`TestConfig::test_transcript_for_stats()`].
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
}

#[derive(Debug)]
struct IndentingWriter<W> {
    inner: W,
    padding: &'static [u8],
    new_line: bool,
}

impl<W: Write> IndentingWriter<W> {
    pub fn new(writer: W, padding: &'static [u8]) -> Self {
        Self {
            inner: writer,
            padding,
            new_line: true,
        }
    }
}

impl<W: Write> Write for IndentingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for (i, line) in buf.split(|&c| c == b'\n').enumerate() {
            if i > 0 {
                self.inner.write_all(b"\n")?;
            }
            if !line.is_empty() && (i > 0 || self.new_line) {
                self.inner.write_all(self.padding)?;
            }
            self.inner.write_all(line)?;
        }
        self.new_line = buf.ends_with(b"\n");
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        svg::{Template, TemplateOptions},
        Captured, UserInput,
    };

    #[test]
    fn indenting_writer_basics() -> io::Result<()> {
        let mut buffer = vec![];
        let mut writer = IndentingWriter::new(&mut buffer, b"  ");
        write!(writer, "Hello, ")?;
        writeln!(writer, "world!")?;
        writeln!(writer, "many\n  lines!")?;

        assert_eq!(buffer, b"  Hello, world!\n  many\n    lines!\n" as &[u8]);
        Ok(())
    }

    fn test_snapshot_testing(test_config: &mut TestConfig) -> anyhow::Result<()> {
        let transcript = Transcript::from_inputs(
            &mut ShellOptions::default(),
            vec![UserInput::command("echo \"Hello, world!\"")],
        )?;

        let mut svg_buffer = vec![];
        Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

        let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
        test_config.test_transcript(&parsed);
        Ok(())
    }

    #[test]
    fn snapshot_testing_with_default_params() -> anyhow::Result<()> {
        let mut test_config = TestConfig::new(ShellOptions::default());
        test_snapshot_testing(&mut test_config)
    }

    #[test]
    fn snapshot_testing_with_exact_match() -> anyhow::Result<()> {
        let mut test_config = TestConfig::new(ShellOptions::default());
        test_snapshot_testing(&mut test_config.with_match_kind(MatchKind::Precise))
    }

    fn test_negative_snapshot_testing(
        out: &mut Vec<u8>,
        test_config: &mut TestConfig,
    ) -> anyhow::Result<()> {
        let mut transcript = Transcript::from_inputs(
            &mut ShellOptions::default(),
            vec![UserInput::command("echo \"Hello, world!\"")],
        )?;
        transcript.add_interaction(UserInput::command("echo \"Sup?\""), "Nah");

        let mut svg_buffer = vec![];
        Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

        let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
        let stats = test_config.test_transcript_inner(&mut NoColor::new(out), &parsed)?;
        assert_eq!(stats.errors(MatchKind::TextOnly), 1);
        Ok(())
    }

    #[test]
    fn negative_snapshot_testing_with_default_output() {
        let mut out = vec![];
        let mut test_config = TestConfig::new(ShellOptions::default());
        test_config.with_color_choice(ColorChoice::Never);
        test_negative_snapshot_testing(&mut out, &mut test_config).unwrap();

        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("[+] Input: echo \"Hello, world!\""), "{}", out);
        assert_eq!(out.matches("Hello, world!").count(), 1, "{}", out);
        // ^ output for successful interactions should not be included
        assert!(out.contains("[-] Input: echo \"Sup?\""), "{}", out);
        assert!(out.contains("Nah"), "{}", out);
    }

    #[test]
    fn negative_snapshot_testing_with_verbose_output() {
        let mut out = vec![];
        let mut test_config = TestConfig::new(ShellOptions::default());
        test_config
            .with_output(TestOutputConfig::Verbose)
            .with_color_choice(ColorChoice::Never);
        test_negative_snapshot_testing(&mut out, &mut test_config).unwrap();

        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("[+] Input: echo \"Hello, world!\""), "{}", out);
        assert_eq!(out.matches("Hello, world!").count(), 2, "{}", out);
        // ^ output for successful interactions should be included
        assert!(out.contains("[-] Input: echo \"Sup?\""), "{}", out);
        assert!(out.contains("Nah"), "{}", out);
    }

    fn diff_snapshot_with_color(
        expected_capture: &str,
        actual_capture: &str,
    ) -> (TestStats, String) {
        let expected_capture = Captured::new(expected_capture.to_owned());
        let parsed = Transcript {
            interactions: vec![Interaction {
                input: UserInput::command("test"),
                output: Parsed {
                    plaintext: expected_capture.to_plaintext().unwrap(),
                    ansi_text: expected_capture.as_ref().to_owned(),
                    html: expected_capture.to_html().unwrap(),
                },
            }],
        };

        let mut reproduced = Transcript::new();
        reproduced.add_interaction(UserInput::command("test"), actual_capture);

        let mut out: Vec<u8> = vec![];
        let stats = TestConfig::new(ShellOptions::default())
            .with_match_kind(MatchKind::Precise)
            .compare_transcripts(&mut NoColor::new(&mut out), &parsed, &reproduced)
            .unwrap();
        (stats, String::from_utf8(out).unwrap())
    }

    #[test]
    fn snapshot_testing_with_color_diff() {
        let (stats, out) = diff_snapshot_with_color(
            "Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m",
            "Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m",
        );

        assert_eq!(stats.matches(), [Some(MatchKind::Precise)]);
        assert!(out.contains("[+] Input: test"), "{}", out);
    }

    #[test]
    fn no_match_for_snapshot_testing_with_color_diff() {
        let (stats, out) = diff_snapshot_with_color(
            "Apr 18 12:54 \u{1b}[0m\u{1b}[33m.\u{1b}[0m",
            "Apr 19 12:54 \u{1b}[0m\u{1b}[33m.\u{1b}[0m",
        );

        assert_eq!(stats.matches(), [None]);
        assert!(out.contains("[-] Input: test"), "{}", out);
    }

    #[test]
    fn text_match_for_snapshot_testing_with_color_diff() {
        let (stats, out) = diff_snapshot_with_color(
            "Apr 18 12:54 \u{1b}[0m\u{1b}[33m.\u{1b}[0m",
            "Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m",
        );

        assert_eq!(stats.matches(), [Some(MatchKind::TextOnly)]);
        assert!(out.contains("[-] Input: test"), "{}", out);
        assert!(out.contains("13..14 ____   yellow/(none)   ____     blue/(none)"));
    }
}
