//! Tests the full lifecycle of `Transcript`s.

use assert_cmd::cargo::CommandCargoExt;

use std::{
    io,
    process::{Command, Stdio},
};

use std::time::Duration;
use term_svg::{
    read_svg_snapshot,
    svg::{Template, TemplateOptions},
    test::{MatchKind, TestConfig, TestOutputConfig},
    ShellOptions, Transcript, UserInput,
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
    Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

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

fn test_transcript_with_empty_output(mute_outputs: &[bool]) -> anyhow::Result<()> {
    #[cfg(unix)]
    const NULL_FILE: &str = "/dev/null";
    #[cfg(windows)]
    const NULL_FILE: &str = "NUL";

    let inputs = mute_outputs.iter().map(|&mute| {
        if mute {
            UserInput::command(format!("rainbow > {}", NULL_FILE))
        } else {
            UserInput::command("rainbow")
        }
    });

    let mut shell_options = ShellOptions::default()
        .with_cargo_path()
        .with_io_timeout(Duration::from_millis(200));
    let transcript = Transcript::from_inputs(&mut shell_options, inputs)?;

    let mut svg_buffer = vec![];
    Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;
    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;

    assert_eq!(parsed.interactions().len(), mute_outputs.len());

    for (interaction, &mute) in parsed.interactions().iter().zip(mute_outputs) {
        if mute {
            assert_eq!(interaction.output().plaintext(), "");
            assert_eq!(interaction.output().html(), "");
        } else {
            assert_ne!(interaction.output().plaintext(), "");
            assert_ne!(interaction.output().html(), "");
        }
    }
    Ok(())
}

#[test]
fn transcript_with_empty_output() -> anyhow::Result<()> {
    test_transcript_with_empty_output(&[true])
}

#[test]
fn transcript_with_empty_and_then_non_empty_outputs() -> anyhow::Result<()> {
    test_transcript_with_empty_output(&[true, false])
}

#[test]
fn transcript_with_non_empty_and_then_empty_outputs() -> anyhow::Result<()> {
    test_transcript_with_empty_output(&[false, true])
}

#[test]
fn transcript_with_sandwiched_empty_output() -> anyhow::Result<()> {
    test_transcript_with_empty_output(&[false, true, false])
}

#[test]
fn transcript_with_sandwiched_non_empty_output() -> anyhow::Result<()> {
    test_transcript_with_empty_output(&[true, false, true])
}

#[test]
fn transcript_with_several_non_empty_outputs_in_succession() -> anyhow::Result<()> {
    test_transcript_with_empty_output(&[true, true, false, true])
}

#[test]
fn failed_shell_initialization() -> anyhow::Result<()> {
    let mut shell_options = ShellOptions::from(Command::cargo_bin("examples/rainbow")?);
    let inputs = vec![UserInput::command("sup")];
    let err = Transcript::from_inputs(&mut shell_options, inputs).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    // We should be able to write all input to the process.

    Ok(())
}

#[test]
fn snapshot_testing() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_svg_snapshot!("rainbow")?)?;
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test]
fn snapshot_testing_with_custom_settings() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_svg_snapshot!("rainbow")?)?;
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test_transcript(&transcript);

    Ok(())
}

#[cfg(unix)]
#[test]
fn sh_shell_example() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_svg_snapshot!("colored-output")?)?;
    let shell_options = ShellOptions::sh().with_alias("colored-output", "examples/rainbow");
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test_transcript(&transcript);

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

    let transcript = Transcript::from_svg(read_svg_snapshot!("colored-output")?)?;
    let shell_options = ShellOptions::bash().with_alias("colored-output", "examples/rainbow");
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test_transcript(&transcript);

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

    let transcript = Transcript::from_svg(read_svg_snapshot!("colored-output")?)?;
    let shell_options = ShellOptions::powershell().with_alias("colored-output", "examples/rainbow");
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test_transcript(&transcript);

    Ok(())
}
