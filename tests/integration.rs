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
fn bash_shell() -> anyhow::Result<()> {
    // Check that the `bash` command exists; exit otherwise.
    let command = Command::new("bash")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match command {
        Ok(status) if status.success() => { /* Success! */ }
        _ => return Ok(()),
    }

    let transcript = include_transcript!("rainbow")?;

    let alias = format!(
        "rainbow() {{ '{}' \"$@\"; }}",
        ShellOptions::cargo_bin("examples/rainbow")
            .to_str()
            .ok_or_else(|| { anyhow::anyhow!("Path to example is not a UTF-8 string") })?,
    );
    let shell_options = ShellOptions::from(Command::new("bash")).with_init_command(alias);

    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}

#[test]
fn powershell() -> anyhow::Result<()> {
    let command = Command::new("powershell")
        .arg("-Help")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match command {
        Ok(status) if status.success() => { /* Success! */ }
        _ => return Ok(()),
    }

    let transcript = read_transcript!("rainbow")?;

    let path_to_example = ShellOptions::cargo_bin("examples/rainbow");
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoLogo").arg("-NoExit");

    let alias_function = format!(
        "function rainbow {{ & '{}' @Args }}",
        path_to_example
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Path to example is not a UTF-8 string"))?
    );
    let shell_options = ShellOptions::from(cmd)
        .with_init_command("function prompt { }")
        .with_init_command(&alias_function)
        .with_line_mapper(|line| {
            if line.starts_with("PS>") {
                None
            } else {
                Some(line)
            }
        });

    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutput::Verbose)
        .test_transcript(&transcript)?
        .assert_no_errors();

    Ok(())
}
