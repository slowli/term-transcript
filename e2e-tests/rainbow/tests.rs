use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
    process::{Command, Stdio},
};

#[cfg(feature = "portable-pty")]
use term_transcript::PtyCommand;
use term_transcript::{
    svg::{NamedPalette, Template, TemplateOptions},
    test::{MatchKind, TestConfig, TestOutputConfig},
    ShellOptions, Transcript, UserInput,
};

const PATH_TO_BIN: &str = env!("CARGO_BIN_EXE_rainbow");
const PATH_TO_REPL_BIN: &str = env!("CARGO_BIN_EXE_rainbow-repl");

fn read_main_snapshot() -> io::Result<BufReader<File>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = manifest_dir.join("../../examples/rainbow.svg");
    File::open(&snapshot_path).map(BufReader::new)
}

fn read_animated_snapshot() -> io::Result<BufReader<File>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = manifest_dir.join("../../examples/animated.svg");
    File::open(&snapshot_path).map(BufReader::new)
}

fn read_aliased_snapshot() -> io::Result<BufReader<File>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = manifest_dir.join("aliased.svg");
    File::open(&snapshot_path).map(BufReader::new)
}

fn read_repl_snapshot() -> io::Result<BufReader<File>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = manifest_dir.join("repl.svg");
    File::open(&snapshot_path).map(BufReader::new)
}

#[test]
fn main_snapshot_can_be_rendered() -> anyhow::Result<()> {
    let mut shell_options = ShellOptions::default().with_cargo_path();
    let transcript =
        Transcript::from_inputs(&mut shell_options, vec![UserInput::command("rainbow")])?;
    let mut buffer = vec![];
    let template_options = TemplateOptions {
        palette: NamedPalette::Gjm8.into(),
        ..TemplateOptions::default()
    };
    Template::new(template_options).render(&transcript, &mut buffer)?;
    let rendered = String::from_utf8(buffer)?;

    let mut snapshot = String::with_capacity(rendered.len());
    read_main_snapshot()?.read_to_string(&mut snapshot)?;

    // Normalize newlines.
    let rendered = rendered.replace("\r\n", "\n");
    let snapshot = snapshot.replace("\r\n", "\n");
    pretty_assertions::assert_eq!(rendered, snapshot);
    Ok(())
}

#[cfg(feature = "portable-pty")]
#[test]
fn main_snapshot_can_be_rendered_from_pty() -> anyhow::Result<()> {
    let mut shell_options = ShellOptions::new(PtyCommand::default()).with_cargo_path();
    let transcript =
        Transcript::from_inputs(&mut shell_options, vec![UserInput::command("rainbow")])?;
    Template::new(TemplateOptions::default()).render(&transcript, io::sink())?;
    Ok(())
}

#[cfg(feature = "portable-pty")]
#[test]
fn snapshot_with_long_lines_can_be_rendered_from_pty() -> anyhow::Result<()> {
    let mut shell_options = ShellOptions::new(PtyCommand::default()).with_cargo_path();
    let transcript = Transcript::from_inputs(
        &mut shell_options,
        vec![UserInput::command("rainbow --long-lines")],
    )?;

    let interaction = &transcript.interactions()[0];
    let output = interaction.output().to_plaintext()?;
    assert!(
        output.contains("\nblack blue green red cyan magenta yellow"),
        "{}",
        output
    );

    Template::new(TemplateOptions::default()).render(&transcript, io::sink())?;
    Ok(())
}

#[test]
fn snapshot_testing() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_main_snapshot()?)?;
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[cfg(feature = "portable-pty")]
#[test]
fn snapshot_testing_with_pty() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_main_snapshot()?)?;
    let shell_options = ShellOptions::new(PtyCommand::default()).with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test]
fn animated_snapshot_testing() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_animated_snapshot()?)?;
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test]
fn snapshot_testing_with_custom_settings() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_main_snapshot()?)?;
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
    let transcript = Transcript::from_svg(read_aliased_snapshot()?)?;
    let shell_options = ShellOptions::sh().with_alias("colored-output", PATH_TO_BIN);
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

    let transcript = Transcript::from_svg(read_aliased_snapshot()?)?;
    let shell_options = ShellOptions::bash().with_alias("colored-output", PATH_TO_BIN);
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

    let transcript = Transcript::from_svg(read_aliased_snapshot()?)?;
    let shell_options = ShellOptions::powershell().with_alias("colored-output", PATH_TO_BIN);
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test_transcript(&transcript);

    Ok(())
}

#[test]
fn repl_snapshot_testing() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_repl_snapshot()?)?;
    let shell_options = ShellOptions::from(Command::new(PATH_TO_REPL_BIN));
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .test_transcript(&transcript);

    Ok(())
}
