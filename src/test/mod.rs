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
//!
//! // Test configuration that can be shared across tests.
//! fn config() -> TestConfig {
//!     let shell_options = ShellOptions::default().with_cargo_path();
//!     TestConfig::new(shell_options)
//!         .with_match_kind(MatchKind::Precise)
//!         .with_output(TestOutputConfig::Verbose)
//! }
//!
//! // Usage in tests:
//! #[test]
//! fn help_command() {
//!     config().test("tests/__snapshots__/help.svg", ["my-command --help"]);
//! }
//! ```
//!
//! Use [`TestConfig::test_transcript()`] for more complex scenarios or increased control:
//!
//! ```
//! use term_transcript::{test::TestConfig, ShellOptions, Transcript, UserInput};
//! # use term_transcript::svg::{Template, TemplateOptions};
//! use std::io;
//!
//! fn read_svg_file() -> anyhow::Result<impl io::BufRead> {
//!     // snipped...
//! #   let transcript = Transcript::from_inputs(
//! #        &mut ShellOptions::default(),
//! #        vec![UserInput::command(r#"echo "Hello world!""#)],
//! #   )?;
//! #   let mut writer = vec![];
//! #   Template::new(TemplateOptions::default()).render(&transcript, &mut writer)?;
//! #   Ok(io::Cursor::new(writer))
//! }
//!
//! # fn main() -> anyhow::Result<()> {
//! let reader = read_svg_file()?;
//! let transcript = Transcript::from_svg(reader)?;
//! TestConfig::new(ShellOptions::default()).test_transcript(&transcript);
//! # Ok(())
//! # }
//! ```

use termcolor::{Color, ColorChoice, ColorSpec, NoColor, StandardStream, WriteColor};

use std::{
    fs::File,
    io::{self, BufReader, Write},
    path::Path,
    process::Command,
    str,
};

use crate::{traits::SpawnShell, Interaction, ShellOptions, TermError, Transcript};

mod color_diff;
mod parser;

use self::color_diff::ColorSpan;
pub use self::parser::{ParseError, Parsed};
#[cfg(feature = "svg")]
use crate::svg::Template;
use crate::{test::color_diff::ColorDiff, UserInput};

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
    #[cfg(feature = "svg")]
    template: Template,
}

impl<Cmd: SpawnShell> TestConfig<Cmd> {
    /// Creates a new config.
    pub fn new(shell_options: ShellOptions<Cmd>) -> Self {
        Self {
            shell_options,
            match_kind: MatchKind::TextOnly,
            output: TestOutputConfig::Normal,
            color_choice: ColorChoice::Auto,
            #[cfg(feature = "svg")]
            template: Template::default(),
        }
    }

    /// Sets the matching kind applied.
    pub fn with_match_kind(mut self, kind: MatchKind) -> Self {
        self.match_kind = kind;
        self
    }

    /// Sets coloring of the output.
    pub fn with_color_choice(mut self, color_choice: ColorChoice) -> Self {
        self.color_choice = color_choice;
        self
    }

    /// Configures test output.
    pub fn with_output(mut self, output: TestOutputConfig) -> Self {
        self.output = output;
        self
    }

    /// Sets the template for rendering new snapshots.
    #[cfg(feature = "svg")]
    #[cfg_attr(docsrs, doc(cfg(feature = "svg")))]
    pub fn with_template(mut self, template: Template) -> Self {
        self.template = template;
        self
    }

    /// Tests a snapshot at the specified path with the provided inputs.
    ///
    /// If the path is relative, it is resolved relative to the current working dir,
    /// which in the case of tests is the root directory of the including crate (i.e., the dir
    /// where the crate manifest is located). Alternatively, you may specify an absolute path
    /// using env vars that Cargo sets during build, such as [`env!("CARGO_MANIFEST_DIR")`].
    ///
    /// Similar to other kinds of snapshot testing, a new snapshot will be generated if
    /// there is no existing snapshot or there are mismatches between inputs or outputs
    /// in the original and reproduced transcripts. This new snapshot will have the same path
    /// as the original snapshot, but with the `.new.svg` extension. As an example,
    /// if the snapshot at `snapshots/help.svg` is tested, the new snapshot will be saved at
    /// `snapshots/help.new.svg`.
    ///
    /// Generation of new snapshots will only happen if the `svg` crate feature is enabled
    /// (which it is by default). The snapshot template can be customized via
    /// [`Self::with_template()`].
    ///
    /// # Panics
    ///
    /// - Panics if there is no snapshot at the specified path, or if the path points
    ///   to a directory.
    /// - Panics if an error occurs during reproducing the transcript or processing
    ///   its output.
    /// - Panics if there are mismatches between inputs or outputs in the original and reproduced
    ///   transcripts.
    ///
    /// [`env!("CARGO_MANIFEST_DIR")`]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    pub fn test<I: Into<UserInput>>(
        &mut self,
        snapshot_path: impl AsRef<Path>,
        inputs: impl IntoIterator<Item = I>,
    ) {
        let inputs = inputs.into_iter().map(Into::into);
        let snapshot_path = snapshot_path.as_ref();

        if snapshot_path.is_file() {
            let snapshot = File::open(snapshot_path).unwrap_or_else(|err| {
                panic!("Cannot open `{:?}`: {}", snapshot_path, err);
            });
            let snapshot = BufReader::new(snapshot);
            let transcript = Transcript::from_svg(snapshot).unwrap_or_else(|err| {
                panic!("Cannot parse snapshot from `{:?}`: {}", snapshot_path, err);
            });
            self.compare_and_test_transcript(
                snapshot_path,
                &transcript,
                &inputs.collect::<Vec<_>>(),
            );
        } else if snapshot_path.exists() {
            panic!(
                "Snapshot path `{:?}` exists, but is not a file",
                snapshot_path
            );
        } else {
            let reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)
                .unwrap_or_else(|err| {
                    panic!("Cannot create a snapshot `{:?}`: {}", snapshot_path, err);
                });
            let new_snapshot_message = self.write_new_snapshot(snapshot_path, &reproduced);
            panic!(
                "Snapshot `{:?}` is missing\n{}",
                snapshot_path, new_snapshot_message
            );
        }
    }

    fn compare_and_test_transcript(
        &mut self,
        snapshot_path: &Path,
        transcript: &Transcript<Parsed>,
        expected_inputs: &[UserInput],
    ) {
        let actual_inputs: Vec<_> = transcript
            .interactions()
            .iter()
            .map(Interaction::input)
            .collect();

        if !actual_inputs.iter().copied().eq(expected_inputs) {
            let reproduced =
                Transcript::from_inputs(&mut self.shell_options, expected_inputs.iter().cloned());
            let reproduced = reproduced.unwrap_or_else(|err| {
                panic!("Cannot create a snapshot `{:?}`: {}", snapshot_path, err);
            });

            let new_snapshot_message = self.write_new_snapshot(snapshot_path, &reproduced);
            panic!(
                "Unexpected user inputs in parsed snapshot: expected {exp:?}, got {act:?}\n{msg}",
                exp = expected_inputs,
                act = actual_inputs,
                msg = new_snapshot_message
            );
        }

        let (stats, reproduced) = self
            .test_transcript_for_stats(transcript)
            .unwrap_or_else(|err| panic!("{}", err));
        if stats.errors(self.match_kind) > 0 {
            let new_snapshot_message = self.write_new_snapshot(snapshot_path, &reproduced);
            panic!("There were test failures\n{}", new_snapshot_message);
        }
    }

    /// Returns message to be appended to the panic message
    #[cfg(feature = "svg")]
    fn write_new_snapshot(&self, path: &Path, transcript: &Transcript) -> String {
        let mut new_path = path.to_owned();
        new_path.set_extension("new.svg");
        let new_snapshot = File::create(&new_path).unwrap_or_else(|err| {
            panic!(
                "Cannot create file for new snapshot `{:?}`: {}",
                new_path, err
            );
        });
        self.template
            .render(transcript, &mut io::BufWriter::new(new_snapshot))
            .unwrap_or_else(|err| {
                panic!("Cannot render snapshot `{:?}`: {}", new_path, err);
            });
        format!("A new snapshot was saved to `{:?}`", new_path)
    }

    #[cfg(not(feature = "svg"))]
    #[allow(clippy::unused_self)] // necessary for uniformity
    fn write_new_snapshot(&self, _: &Path, _: &Transcript) -> String {
        format!(
            "Not writing a new snapshot since `{}/svg` feature is not enabled",
            env!("CARGO_CRATE_NAME")
        )
    }

    /// Tests the `transcript`. This is a lower-level alternative to [`Self::test()`].
    ///
    /// # Panics
    ///
    /// - Panics if an error occurs during reproducing the transcript or processing
    ///   its output.
    /// - Panics if there are mismatches between outputs in the original and reproduced
    ///   transcripts.
    pub fn test_transcript(&mut self, transcript: &Transcript<Parsed>) {
        let (stats, _) = self
            .test_transcript_for_stats(transcript)
            .unwrap_or_else(|err| panic!("{}", err));
        stats.assert_no_errors(self.match_kind);
    }

    /// Tests the `transcript` and returns testing stats together with
    /// the reproduced [`Transcript`]. This is a lower-level alternative to [`Self::test()`].
    ///
    /// # Errors
    ///
    /// - Returns an error if an error occurs during reproducing the transcript or processing
    ///   its output.
    pub fn test_transcript_for_stats(
        &mut self,
        transcript: &Transcript<Parsed>,
    ) -> io::Result<(TestStats, Transcript)> {
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
    ) -> io::Result<(TestStats, Transcript)> {
        let inputs = transcript
            .interactions()
            .iter()
            .map(|interaction| interaction.input().clone());
        let reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)?;

        let stats = self.compare_transcripts(out, transcript, &reproduced)?;
        Ok((stats, reproduced))
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
                let original_spans = &original.output().color_spans;
                let reproduced_spans =
                    ColorSpan::parse(reproduced.as_ref()).map_err(|err| match err {
                        TermError::Io(err) => err,
                        other => io::Error::new(io::ErrorKind::InvalidInput, other),
                    })?;

                let diff = ColorDiff::new(original_spans, &reproduced_spans);
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
                if color_diff.is_some() {
                    write!(out, "#")?;
                } else {
                    write!(out, "-")?;
                }
            }
            out.set_color(ColorSpec::new().set_intense(true))?;
            write!(out, "]")?;
            out.reset()?;
            writeln!(out, " Input: {}", original.input().as_ref())?;

            if let Some(diff) = color_diff {
                let original_spans = &original.output().color_spans;
                diff.highlight_text(out, original_text, original_spans)?;
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
        let test_config = TestConfig::new(ShellOptions::default());
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
        let (stats, _) = test_config.test_transcript_inner(&mut NoColor::new(out), &parsed)?;
        assert_eq!(stats.errors(MatchKind::TextOnly), 1);
        Ok(())
    }

    #[test]
    fn negative_snapshot_testing_with_default_output() {
        let mut out = vec![];
        let mut test_config =
            TestConfig::new(ShellOptions::default()).with_color_choice(ColorChoice::Never);
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
        let mut test_config = TestConfig::new(ShellOptions::default())
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
                    color_spans: ColorSpan::parse(expected_capture.as_ref()).unwrap(),
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
        assert!(out.contains("[#] Input: test"), "{}", out);
        assert!(out.contains("13..14 ____   yellow/(none)   ____     blue/(none)"));
    }
}
