//! Implementation details for `TestConfig`.

use std::{
    fmt,
    fs::File,
    io::{self, BufReader, Write},
    path::Path,
    str,
};

use termcolor::{Color, ColorSpec, NoColor, WriteColor};

use super::{
    color_diff::{ColorDiff, ColorSpan},
    parser::Parsed,
    utils::{ColorPrintlnWriter, IndentingWriter},
    MatchKind, TestConfig, TestOutputConfig, TestStats,
};
use crate::{traits::SpawnShell, Interaction, TermError, Transcript, UserInput};

impl<Cmd: SpawnShell + fmt::Debug, F: FnMut(&mut Transcript)> TestConfig<Cmd, F> {
    /// Tests a snapshot at the specified path with the provided inputs.
    ///
    /// If the path is relative, it is resolved relative to the current working dir,
    /// which in the case of tests is the root directory of the including crate (i.e., the dir
    /// where the crate manifest is located). You may specify an absolute path
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
    /// (which it is by default), and if the [update mode](Self::with_update_mode())
    /// is not [`UpdateMode::Never`], either because it was set explicitly or
    /// [inferred] from the execution environment.
    ///
    /// The snapshot template can be customized via [`Self::with_template()`].
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
    /// [`UpdateMode::Never`]: crate::test::UpdateMode::Never
    /// [inferred]: crate::test::UpdateMode::from_env()
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(skip_all, fields(snapshot_path, inputs))
    )]
    pub fn test<I: Into<UserInput>>(
        &mut self,
        snapshot_path: impl AsRef<Path>,
        inputs: impl IntoIterator<Item = I>,
    ) {
        let inputs: Vec<_> = inputs.into_iter().map(Into::into).collect();
        let snapshot_path = snapshot_path.as_ref();
        #[cfg(feature = "tracing")]
        tracing::Span::current()
            .record("snapshot_path", tracing::field::debug(snapshot_path))
            .record("inputs", tracing::field::debug(&inputs));

        if snapshot_path.is_file() {
            #[cfg(feature = "tracing")]
            tracing::debug!(snapshot_path.is_file = true);

            let snapshot = File::open(snapshot_path).unwrap_or_else(|err| {
                panic!("Cannot open `{}`: {err}", snapshot_path.display());
            });
            let snapshot = BufReader::new(snapshot);
            let transcript = Transcript::from_svg(snapshot).unwrap_or_else(|err| {
                panic!(
                    "Cannot parse snapshot from `{}`: {err}",
                    snapshot_path.display()
                );
            });
            self.compare_and_test_transcript(snapshot_path, &transcript, &inputs);
        } else if snapshot_path.exists() {
            panic!(
                "Snapshot path `{}` exists, but is not a file",
                snapshot_path.display()
            );
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(snapshot_path.is_file = false);

            let new_snapshot_message =
                self.create_and_write_new_snapshot(snapshot_path, inputs.into_iter());
            panic!(
                "Snapshot `{}` is missing\n{new_snapshot_message}",
                snapshot_path.display()
            );
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self, transcript))
    )]
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
            let new_snapshot_message =
                self.create_and_write_new_snapshot(snapshot_path, expected_inputs.iter().cloned());
            panic!(
                "Unexpected user inputs in parsed snapshot: expected {expected_inputs:?}, \
                 got {actual_inputs:?}\n{new_snapshot_message}"
            );
        }

        let (stats, reproduced) = self
            .test_transcript_for_stats(transcript)
            .unwrap_or_else(|err| panic!("{err}"));
        if stats.errors(self.match_kind) > 0 {
            let new_snapshot_message = self.write_new_snapshot(snapshot_path, &reproduced);
            panic!("There were test failures\n{new_snapshot_message}");
        }
    }

    #[cfg(feature = "svg")]
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self, inputs))
    )]
    fn create_and_write_new_snapshot(
        &mut self,
        path: &Path,
        inputs: impl Iterator<Item = UserInput>,
    ) -> String {
        let mut reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)
            .unwrap_or_else(|err| {
                panic!("Cannot create a snapshot `{}`: {err}", path.display());
            });
        (self.transform)(&mut reproduced);
        self.write_new_snapshot(path, &reproduced)
    }

    /// Returns a message to be appended to the panic message.
    #[cfg(feature = "svg")]
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self, transcript), ret)
    )]
    fn write_new_snapshot(&self, path: &Path, transcript: &Transcript) -> String {
        if !self.update_mode.should_create_snapshot() {
            return format!(
                "Skipped writing new snapshot `{}` per test config",
                path.display()
            );
        }

        let mut new_path = path.to_owned();
        new_path.set_extension("new.svg");
        let new_snapshot = File::create(&new_path).unwrap_or_else(|err| {
            panic!(
                "Cannot create file for new snapshot `{}`: {err}",
                new_path.display()
            );
        });
        self.template
            .render(transcript, &mut io::BufWriter::new(new_snapshot))
            .unwrap_or_else(|err| {
                panic!("Cannot render snapshot `{}`: {err}", new_path.display());
            });
        format!("A new snapshot was saved to `{}`", new_path.display())
    }

    #[cfg(not(feature = "svg"))]
    #[allow(clippy::unused_self)] // necessary for uniformity
    fn write_new_snapshot(&self, _: &Path, _: &Transcript) -> String {
        format!(
            "Not writing a new snapshot since `{}/svg` feature is not enabled",
            env!("CARGO_PKG_NAME")
        )
    }

    #[cfg(not(feature = "svg"))]
    #[allow(clippy::unused_self)] // necessary for uniformity
    fn create_and_write_new_snapshot(
        &mut self,
        _: &Path,
        _: impl Iterator<Item = UserInput>,
    ) -> String {
        format!(
            "Not writing a new snapshot since `{}/svg` feature is not enabled",
            env!("CARGO_PKG_NAME")
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
            .unwrap_or_else(|err| panic!("{err}"));
        stats.assert_no_errors(self.match_kind);
    }

    /// Tests the `transcript` and returns testing stats together with
    /// the reproduced [`Transcript`]. This is a lower-level alternative to [`Self::test()`].
    ///
    /// # Errors
    ///
    /// - Returns an error if an error occurs during reproducing the transcript or processing
    ///   its output.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, err))]
    pub fn test_transcript_for_stats(
        &mut self,
        transcript: &Transcript<Parsed>,
    ) -> io::Result<(TestStats, Transcript)> {
        if self.output == TestOutputConfig::Quiet {
            let mut out = NoColor::new(io::sink());
            self.test_transcript_inner(&mut out, transcript)
        } else {
            let mut out = ColorPrintlnWriter::new(self.color_choice);
            self.test_transcript_inner(&mut out, transcript)
        }
    }

    pub(super) fn test_transcript_inner(
        &mut self,
        out: &mut impl WriteColor,
        transcript: &Transcript<Parsed>,
    ) -> io::Result<(TestStats, Transcript)> {
        let inputs = transcript
            .interactions()
            .iter()
            .map(|interaction| interaction.input().clone());
        let mut reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)?;
        (self.transform)(&mut reproduced);

        let stats = self.compare_transcripts(out, transcript, &reproduced)?;
        Ok((stats, reproduced))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, ret, err))]
    pub(super) fn compare_transcripts(
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
            #[cfg(feature = "tracing")]
            let _entered =
                tracing::debug_span!("compare_interaction", input = ?original.input).entered();

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
            #[cfg(feature = "tracing")]
            tracing::debug!(?actual_match, "compared output texts");

            // If we do precise matching, check it as well.
            let color_diff = if self.match_kind == MatchKind::Precise && actual_match.is_some() {
                let original_spans = &original.output().color_spans;
                let reproduced_spans =
                    ColorSpan::parse(reproduced.as_ref()).map_err(|err| match err {
                        TermError::Io(err) => err,
                        other => io::Error::new(io::ErrorKind::InvalidInput, other),
                    })?;

                let diff = ColorDiff::new(original_spans, &reproduced_spans);
                #[cfg(feature = "tracing")]
                tracing::debug!(?diff, "compared output coloring");

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

        // Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
        // to switch to `fmt::Display`.
        struct DebugStr<'a>(&'a str);

        impl fmt::Debug for DebugStr<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                // Align output with verbose term output. Since `Comparison` adds one space,
                // we need to add 3 spaces instead of 4.
                for line in self.0.lines() {
                    writeln!(formatter, "   {line}")?;
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
            writeln!(out, "    {line}")?;
        }
        writeln!(out, "  Reproduced:")?;
        for line in reproduced.lines() {
            writeln!(out, "    {line}")?;
        }
        Ok(())
    }
}
