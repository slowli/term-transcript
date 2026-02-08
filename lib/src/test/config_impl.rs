//! Implementation details for `TestConfig`.

use std::{
    fmt,
    fs::File,
    io::{self, BufReader, Write},
    path::Path,
};

use anstream::{AutoStream, ColorChoice};
use anstyle::{Ansi256Color, AnsiColor, Color, Style};
use term_style::{StyleDiff, TextDiff};

use super::{
    MatchKind, TestConfig, TestOutputConfig, TestStats,
    utils::{ChoiceWriter, IndentingWriter, PrintlnWriter},
};
use crate::{Interaction, Transcript, UserInput, traits::SpawnShell};

const SUCCESS: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightGreen)));
const ERROR: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightRed)));
// medium gray
const VERBOSE_OUTPUT: Style = Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(244))));

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all, ret, err))]
#[doc(hidden)] // low-level; not public API
pub fn compare_transcripts(
    out: &mut impl Write,
    parsed: &Transcript,
    reproduced: &Transcript,
    match_kind: MatchKind,
    verbose: bool,
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

        write!(out, "  [")?;

        // First, process text only.
        let original_text = original.output().text();
        let reproduced_text = reproduced.text();

        let mut actual_match = if original_text == reproduced_text {
            Some(MatchKind::TextOnly)
        } else {
            None
        };
        #[cfg(feature = "tracing")]
        tracing::debug!(?actual_match, "compared output texts");

        // If we do precise matching, check it as well.
        let color_diff = if match_kind == MatchKind::Precise && actual_match.is_some() {
            let diff = StyleDiff::new(original.output().as_ref(), reproduced.as_ref());
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
        if actual_match >= Some(match_kind) {
            write!(out, "{SUCCESS}+{SUCCESS:#}")?;
        } else if color_diff.is_some() {
            write!(out, "{ERROR}#{ERROR:#}")?;
        } else {
            write!(out, "{ERROR}-{ERROR:#}")?;
        }
        writeln!(out, "] Input: {}", original.input().as_ref())?;

        if let Some(diff) = color_diff {
            write!(out, "{diff:>4}{diff:#}")?;
        } else if actual_match.is_none() {
            write!(out, "{:>4}", TextDiff::new(original_text, reproduced_text))?;
        } else if verbose {
            write!(out, "{VERBOSE_OUTPUT}")?;
            let mut out_with_indents = IndentingWriter::new(&mut *out, "    ");
            writeln!(out_with_indents, "{}", original.output().text())?;
            write!(out, "{VERBOSE_OUTPUT:#}")?;
        }
    }

    out.flush()?; // apply terminal styling if necessary
    Ok(stats)
}

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
        transcript: &Transcript,
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
    pub fn test_transcript(&mut self, transcript: &Transcript) {
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
        transcript: &Transcript,
    ) -> io::Result<(TestStats, Transcript)> {
        if self.output == TestOutputConfig::Quiet {
            self.test_transcript_inner(&mut io::sink(), transcript)
        } else {
            let choice = if self.color_choice == ColorChoice::Auto {
                AutoStream::choice(&io::stdout())
            } else {
                self.color_choice
            };
            // We cannot create an `AutoStream` here because it would require `PrintlnWriter` to implement `anstream::RawStream`,
            // which is a sealed trait.
            let mut out = ChoiceWriter::new(PrintlnWriter::default(), choice);
            self.test_transcript_inner(&mut out, transcript)
        }
    }

    pub(super) fn test_transcript_inner(
        &mut self,
        out: &mut impl Write,
        transcript: &Transcript,
    ) -> io::Result<(TestStats, Transcript)> {
        let inputs = transcript
            .interactions()
            .iter()
            .map(|interaction| interaction.input().clone());
        let mut reproduced = Transcript::from_inputs(&mut self.shell_options, inputs)?;
        (self.transform)(&mut reproduced);

        let stats = compare_transcripts(
            out,
            transcript,
            &reproduced,
            self.match_kind,
            self.output == TestOutputConfig::Verbose,
        )?;
        Ok((stats, reproduced))
    }
}
