//! Tests the full lifecycle of `Transcript`s.

use assert_matches::assert_matches;

use std::{
    io,
    path::Path,
    process::{Command, Stdio},
    str::Utf8Error,
    time::Duration,
};

use term_transcript::{
    svg::{Template, TemplateOptions},
    ShellOptions, Transcript, UserInput,
};

#[cfg(unix)]
fn echo_command() -> Command {
    let mut command = Command::new("echo");
    command.arg("Hello, world!");
    command
}

#[cfg(windows)]
fn echo_command() -> Command {
    let mut command = Command::new("cmd");
    command.arg("/Q").arg("/C").arg("echo Hello, world!");
    command
}

#[test]
fn transcript_lifecycle() -> anyhow::Result<()> {
    let mut transcript = Transcript::new();

    // 1. Capture output from a command.
    transcript.capture_output(
        UserInput::command("echo \"Hello, world!\""),
        &mut echo_command(),
    )?;

    // 2. Render the transcript into SVG.
    let mut svg_buffer = vec![];
    Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

    // 3. Parse SVG back to the transcript.
    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    assert_eq!(parsed.interactions().len(), 1);
    let interaction = &parsed.interactions()[0];
    assert_eq!(
        *interaction.input(),
        UserInput::command("echo \"Hello, world!\"")
    );

    // 4. Compare output to the output in the original transcript.
    assert_eq!(
        interaction.output().plaintext(),
        transcript.interactions()[0].output().to_plaintext()?
    );
    assert_eq!(
        interaction.output().html(),
        transcript.interactions()[0].output().to_html()?
    );
    Ok(())
}

fn test_transcript_with_empty_output(mute_outputs: &[bool]) -> anyhow::Result<()> {
    #[cfg(unix)]
    const NULL_FILE: &str = "/dev/null";
    #[cfg(windows)]
    const NULL_FILE: &str = "NUL";

    let inputs = mute_outputs.iter().map(|&mute| {
        if mute {
            UserInput::command(format!("echo \"Hello, world!\" > {NULL_FILE}"))
        } else {
            UserInput::command("echo \"Hello, world!\"")
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
#[ignore] // TODO: investigate this test fails in CI
fn failed_shell_initialization() {
    let inputs = vec![UserInput::command("sup")];
    let err = Transcript::from_inputs(&mut echo_command().into(), inputs).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    // We should not be able to write all input to the process.
}

#[cfg(unix)]
#[test]
fn command_exit_status_in_sh() -> anyhow::Result<()> {
    let mut options = ShellOptions::sh();
    // ^ The error output is locale-specific and is not always UTF-8
    let inputs = [
        UserInput::command("echo \"Hello world!\""),
        UserInput::command("some-command-that-should-never-exist"),
    ];
    let transcript = Transcript::from_inputs(&mut options, inputs)?;

    let exit_status = transcript.interactions()[0].exit_status().unwrap();
    assert!(exit_status.is_success(), "{exit_status:?}");
    let exit_status = transcript.interactions()[1].exit_status().unwrap();
    assert!(!exit_status.is_success(), "{exit_status:?}");
    Ok(())
}

#[test]
fn command_exit_status_in_powershell() -> anyhow::Result<()> {
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

    let mut options = ShellOptions::powershell()
        .with_init_timeout(Duration::from_secs(2))
        .with_lossy_utf8_decoder();
    // ^ The error output is locale-specific and is not always UTF-8
    let inputs = [
        UserInput::command("echo \"Hello world!\""),
        UserInput::command("some-command-that-should-never-exist"),
    ];
    let transcript = Transcript::from_inputs(&mut options, inputs)?;

    let exit_status = transcript.interactions()[0].exit_status().unwrap();
    assert!(exit_status.is_success(), "{exit_status:?}");
    let exit_status = transcript.interactions()[1].exit_status().unwrap();
    assert!(!exit_status.is_success(), "{exit_status:?}");
    Ok(())
}

/// The default `cmd` codepage can lead to non-UTF8 output for builtin commands
/// (e.g., `dir` may output non-breakable space in file sizes as 0xff).
/// Here, we test that the codepage is switched to UTF-8.
#[cfg(windows)]
#[test]
fn cmd_shell_with_utf8_output() {
    let input = UserInput::command(format!("dir {}", env!("CARGO_MANIFEST_DIR")));
    let transcript = Transcript::from_inputs(&mut ShellOptions::default(), vec![input]).unwrap();

    assert_eq!(transcript.interactions().len(), 1);
    let output = transcript.interactions()[0].output().as_ref();
    assert!(output.contains("LICENSE-APACHE"));
    assert!(!output.contains('\r'));
}

#[cfg(all(windows, feature = "portable-pty"))]
#[test]
fn cmd_shell_with_utf8_output_in_pty() {
    use term_transcript::PtyCommand;

    let input = UserInput::command(format!("dir {}", env!("CARGO_MANIFEST_DIR")));
    let mut options = ShellOptions::new(PtyCommand::default());
    let transcript = Transcript::from_inputs(&mut options, vec![input]).unwrap();

    assert_eq!(transcript.interactions().len(), 1);
    let output = transcript.interactions()[0].output().as_ref();
    assert!(output.contains("LICENSE-APACHE"));
    assert!(output.lines().all(|line| !line.ends_with('\r')));

    // Check that the captured output can be rendered.
    Template::new(TemplateOptions::default())
        .render(&transcript, &mut vec![])
        .unwrap();
}

#[test]
fn non_utf8_shell_output() {
    #[cfg(unix)]
    const CAT_COMMAND: &str = "cat";
    #[cfg(windows)]
    const CAT_COMMAND: &str = "type";

    let non_utf8_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("non-utf8.txt");
    let input = UserInput::command(format!(
        "{CAT_COMMAND} \"{}\"",
        non_utf8_file.to_string_lossy()
    ));
    let err = Transcript::from_inputs(&mut ShellOptions::default(), vec![input]).unwrap_err();

    assert_matches!(err.kind(), io::ErrorKind::InvalidData);
    assert!(err.get_ref().unwrap().is::<Utf8Error>(), "{err:?}");
}

#[test]
fn non_utf8_shell_output_with_lossy_decoder() -> anyhow::Result<()> {
    #[cfg(unix)]
    const CAT_COMMAND: &str = "cat";
    #[cfg(windows)]
    const CAT_COMMAND: &str = "type";

    let non_utf8_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("non-utf8.txt");
    let input = UserInput::command(format!(
        "{CAT_COMMAND} \"{}\"",
        non_utf8_file.to_string_lossy()
    ));

    let mut options = ShellOptions::default().with_lossy_utf8_decoder();
    let transcript = Transcript::from_inputs(&mut options, vec![input])?;
    let output = transcript.interactions()[0].output();
    assert!(output.to_plaintext()?.contains(char::REPLACEMENT_CHARACTER));
    Ok(())
}
