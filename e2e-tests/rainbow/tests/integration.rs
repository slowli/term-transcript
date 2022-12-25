use handlebars::Template as HandlebarsTemplate;
use tempfile::tempdir;
use tracing::{subscriber::DefaultGuard, Subscriber};
use tracing_subscriber::{fmt::format::FmtSpan, FmtSubscriber};

use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    panic,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

#[cfg(feature = "portable-pty")]
use term_transcript::PtyCommand;
use term_transcript::{
    svg::{NamedPalette, Template, TemplateOptions},
    test::{MatchKind, TestConfig, TestOutputConfig, UpdateMode},
    ShellOptions, Transcript, UserInput,
};

const PATH_TO_BIN: &str = env!("CARGO_BIN_EXE_rainbow");
const PATH_TO_REPL_BIN: &str = env!("CARGO_BIN_EXE_rainbow-repl");

fn create_fmt_subscriber() -> impl Subscriber {
    FmtSubscriber::builder()
        .pretty()
        .with_span_events(FmtSpan::CLOSE)
        .with_test_writer()
        .with_env_filter("term_transcript=debug")
        .finish()
}

fn enable_tracing() -> DefaultGuard {
    tracing::subscriber::set_default(create_fmt_subscriber())
}

fn main_snapshot_path() -> &'static Path {
    Path::new("../../examples/rainbow.svg")
}

fn read_main_snapshot() -> io::Result<BufReader<File>> {
    File::open(main_snapshot_path()).map(BufReader::new)
}

fn read_custom_template() -> anyhow::Result<HandlebarsTemplate> {
    let template_string = fs::read_to_string(Path::new("../../examples/custom.html.handlebars"))?;
    HandlebarsTemplate::compile(&template_string).map_err(Into::into)
}

fn animated_snapshot_path() -> &'static Path {
    Path::new("../../examples/animated.svg")
}

fn aliased_snapshot_path() -> &'static Path {
    Path::new("aliased.svg")
}

fn repl_snapshot_path() -> &'static Path {
    Path::new("repl.svg")
}

#[test]
fn main_snapshot_can_be_rendered() -> anyhow::Result<()> {
    let _guard = enable_tracing();
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

#[test]
fn snapshot_with_custom_template() -> anyhow::Result<()> {
    let _guard = enable_tracing();
    let mut shell_options = ShellOptions::default().with_cargo_path();
    let transcript =
        Transcript::from_inputs(&mut shell_options, vec![UserInput::command("rainbow")])?;
    let template = read_custom_template()?;

    let template_options = TemplateOptions {
        palette: NamedPalette::Gjm8.into(),
        ..TemplateOptions::default()
    };
    let mut buffer = vec![];
    Template::custom(template, template_options).render(&transcript, &mut buffer)?;
    let buffer = String::from_utf8(buffer)?;
    assert!(buffer.starts_with("<!DOCTYPE html>"), "{buffer}");
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
        "{output}"
    );

    Template::new(TemplateOptions::default()).render(&transcript, io::sink())?;
    Ok(())
}

#[test]
fn snapshot_testing_low_level() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_main_snapshot()?)?;
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test]
fn snapshot_testing() {
    let _guard = enable_tracing();
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options).test(main_snapshot_path(), ["rainbow"]);
}

#[cfg(feature = "portable-pty")]
#[test]
fn snapshot_testing_with_pty() {
    let shell_options = ShellOptions::new(PtyCommand::default()).with_cargo_path();
    TestConfig::new(shell_options).test(main_snapshot_path(), ["rainbow"]);
}

#[test]
fn animated_snapshot_testing() {
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options).test(
        animated_snapshot_path(),
        ["rainbow", "rainbow --long-lines"],
    );
}

#[test]
fn snapshot_testing_with_custom_settings() {
    let shell_options = ShellOptions::default().with_cargo_path();
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test(main_snapshot_path(), ["rainbow"]);
}

#[cfg(unix)]
#[test]
fn sh_shell_example() {
    let shell_options = ShellOptions::sh().with_alias("colored-output", PATH_TO_BIN);
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test(aliased_snapshot_path(), ["colored-output"]);
}

#[cfg(unix)]
// Although `bash` can be present on Windows, `with_alias` will most probably work
// improperly because of Windows-style paths.
#[test]
fn bash_shell_example() {
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
        return;
    }

    let shell_options = ShellOptions::bash().with_alias("colored-output", PATH_TO_BIN);
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test(aliased_snapshot_path(), ["colored-output"]);
}

#[test]
fn powershell_example() {
    fn powershell_exists() -> bool {
        let exit_status = Command::new("pwsh")
            .arg("-Help")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        matches!(exit_status, Ok(status) if status.success())
    }

    if !powershell_exists() {
        println!("pwsh not found; exiting");
        return;
    }

    let shell_options = ShellOptions::pwsh()
        .with_init_timeout(Duration::from_secs(2))
        .with_alias("colored-output", PATH_TO_BIN);
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose)
        .test(aliased_snapshot_path(), ["colored-output"]);
}

#[test]
fn repl_snapshot_testing() {
    let shell_options = ShellOptions::from(Command::new(PATH_TO_REPL_BIN));
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .test(
            repl_snapshot_path(),
            [
                "yellow intense bold green cucumber",
                "neutral #fa4 underline #c0ffee",
                "#9f4010 (brown) italic",
            ],
        );
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ErrorType {
    MissingSnapshot,
    InputMismatch,
    OutputMismatch,
}

impl ErrorType {
    fn create_snapshot(self, snapshot_path: &Path) -> io::Result<()> {
        match self {
            Self::MissingSnapshot => {
                Ok(()) // Do nothing.
            }
            Self::InputMismatch => {
                let mut buffer = String::new();
                read_main_snapshot()?.read_to_string(&mut buffer)?;
                let buffer = buffer.replace(" rainbow", " ????");
                fs::write(snapshot_path, buffer)
            }
            Self::OutputMismatch => {
                let mut buffer = String::new();
                read_main_snapshot()?.read_to_string(&mut buffer)?;
                let buffer = buffer.replace("pink", "???");
                fs::write(snapshot_path, buffer)
            }
        }
    }

    fn expected_error_message(self) -> &'static str {
        match self {
            Self::MissingSnapshot => "is missing",
            Self::InputMismatch => "Unexpected user inputs",
            Self::OutputMismatch => "There were test failures",
        }
    }
}

fn test_new_snapshot(error_type: ErrorType) -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let snapshot_path = temp_dir.path().join("rainbow.svg");
    error_type.create_snapshot(&snapshot_path)?;

    let test_result = panic::catch_unwind(|| {
        let shell_options = ShellOptions::default().with_cargo_path();
        TestConfig::new(shell_options)
            .with_update_mode(UpdateMode::Always)
            .test(&snapshot_path, ["rainbow"]);
    });

    let err = *test_result.unwrap_err().downcast::<String>().unwrap();
    assert!(
        err.contains(error_type.expected_error_message()),
        "Unexpected error message: {err}"
    );
    assert!(
        err.contains("rainbow.new.svg"),
        "Unexpected error message: {err}"
    );

    let new_snapshot_path = temp_dir.path().join("rainbow.new.svg");
    let new_snapshot_file = BufReader::new(File::open(new_snapshot_path)?);
    let new_transcript = Transcript::from_svg(new_snapshot_file)?;

    let interactions = new_transcript.interactions();
    assert_eq!(interactions.len(), 1);
    let output_plaintext = interactions[0].output().plaintext();
    assert!(
        output_plaintext.contains("pink"),
        "Unexpected output: {output_plaintext}"
    );

    Ok(())
}

#[test]
fn new_snapshot_is_created_if_original_is_missing() -> anyhow::Result<()> {
    test_new_snapshot(ErrorType::MissingSnapshot)
}

#[test]
fn new_snapshot_is_created_on_input_mismatch() -> anyhow::Result<()> {
    test_new_snapshot(ErrorType::InputMismatch)
}

#[test]
fn new_snapshot_is_created_on_output_mismatch() -> anyhow::Result<()> {
    test_new_snapshot(ErrorType::OutputMismatch)
}

fn test_no_new_snapshot(error_type: ErrorType) -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let snapshot_path = temp_dir.path().join("rainbow.svg");
    error_type.create_snapshot(&snapshot_path)?;

    let test_result = panic::catch_unwind(|| {
        let shell_options = ShellOptions::default().with_cargo_path();
        TestConfig::new(shell_options)
            .with_update_mode(UpdateMode::Never)
            .test(&snapshot_path, ["rainbow"]);
    });

    let err = *test_result.unwrap_err().downcast::<String>().unwrap();
    assert!(
        err.contains(error_type.expected_error_message()),
        "Unexpected error message: {err}"
    );
    if error_type != ErrorType::MissingSnapshot {
        assert!(
            err.contains("Skipped writing new snapshot"),
            "Unexpected error message: {err}"
        );
    }

    let new_snapshot_path = temp_dir.path().join("rainbow.new.svg");
    assert!(!new_snapshot_path.exists());

    Ok(())
}

#[test]
fn new_snapshot_is_not_created_with_config_if_original_is_missing() -> anyhow::Result<()> {
    test_no_new_snapshot(ErrorType::MissingSnapshot)
}

#[test]
fn new_snapshot_is_not_created_with_config_on_input_mismatch() -> anyhow::Result<()> {
    test_no_new_snapshot(ErrorType::InputMismatch)
}

#[test]
fn new_snapshot_is_not_created_with_config_on_output_mismatch() -> anyhow::Result<()> {
    test_no_new_snapshot(ErrorType::OutputMismatch)
}
