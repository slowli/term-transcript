//! Tests the full lifecycle of `Transcript`s.

use assert_cmd::cargo::CommandCargoExt;

use std::process::{Command, Stdio};

use term_svg::{
    read_transcript,
    test::{TestConfig, TestOutput},
    MatchKind, ShellOptions, SvgTemplate, SvgTemplateOptions, Transcript, UserInput,
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

#[test]
fn snapshot_testing() -> anyhow::Result<()> {
    let transcript = read_transcript!("rainbow")?;
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}

#[cfg(unix)]
#[test]
fn sh_shell_example() -> anyhow::Result<()> {
    let transcript = read_transcript!("colored-output")?;
    let shell_options = ShellOptions::sh().with_alias("colored-output", "examples/rainbow");
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}

#[cfg(unix)]
// Although `bash` can be present on Windows, `with_alias` will most probably work
// improperly because of Windows-style paths.
#[test]
fn bash_shell_example() -> anyhow::Result<()> {
    fn bash_exists() -> bool {
        let exit_status = Command::new("bash")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        matches!(exit_status, Ok(status) if status.success())
    }

    if !bash_exists() {
        println!("bash not found; skipping");
        return Ok(());
    }

    let transcript = read_transcript!("colored-output")?;
    let shell_options = ShellOptions::bash().with_alias("colored-output", "examples/rainbow");
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}

#[test]
fn powershell_example() -> anyhow::Result<()> {
    fn powershell_exists() -> bool {
        let exit_status = Command::new("powershell")
            .arg("-Help")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        matches!(exit_status, Ok(status) if status.success())
    }

    if !powershell_exists() {
        println!("powershell not found; exiting");
        return Ok(());
    }

    let transcript = read_transcript!("colored-output")?;
    let shell_options = ShellOptions::powershell().with_alias("colored-output", "examples/rainbow");
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}
