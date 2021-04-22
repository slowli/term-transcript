//! Tests the full lifecycle of `Transcript`s.

use assert_cmd::cargo::CommandCargoExt;

use std::{fs::File, io::BufReader, path::Path, process::Command};

use term_svg::test::TestOutput;
use term_svg::{
    test::TestConfig, MatchKind, ShellOptions, SvgTemplate, SvgTemplateOptions, Transcript,
    UserInput,
};

#[test]
fn transcript_lifecycle() -> anyhow::Result<()> {
    let mut transcript = Transcript::new();

    // 1. Capture output from a command.
    transcript.capture_output(
        UserInput::command("rainbow"),
        &mut Command::cargo_bin("examples/rainbow")?,
    )?;

    // 2. Render the transcript into SVG.
    let mut svg_buffer = vec![];
    SvgTemplate::new(SvgTemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

    // 3. Parse SVG back to the transcript.
    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    assert_eq!(parsed.interactions().len(), 1);
    let interaction = &parsed.interactions()[0];
    assert_eq!(*interaction.input(), UserInput::command("rainbow"));

    // 4. Compare output to the output in the original transcript.
    interaction
        .output()
        .assert_matches(transcript.interactions()[0].output(), MatchKind::Precise);

    Ok(())
}

#[cfg(any(unix, windows))]
#[test]
fn snapshot_testing() -> anyhow::Result<()> {
    let snapshot_path = Path::new(file!())
        .parent()
        .expect("No parent of current file")
        .join("snapshots/rainbow.svg");
    let svg = BufReader::new(File::open(snapshot_path)?);
    let transcript = Transcript::from_svg(svg)?;

    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}
