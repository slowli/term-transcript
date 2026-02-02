use std::{
    env,
    fs::{self, File},
    io::{self, BufReader, Read},
    panic,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use handlebars::Template as HandlebarsTemplate;
use tempfile::tempdir;
#[cfg(feature = "portable-pty")]
use term_transcript::PtyCommand;
use term_transcript::{
    svg::{NamedPalette, Template, TemplateOptions, ValidTemplateOptions},
    test::{compare_transcripts, MatchKind, TestConfig, TestOutputConfig, UpdateMode},
    ExitStatus, ShellOptions, Transcript, UserInput,
};
use test_casing::{
    decorate,
    decorators::{Retry, Trace},
    test_casing, Product,
};

const PATH_TO_REPL_BIN: &str = env!("CARGO_BIN_EXE_rainbow-repl");

static TRACING: Trace = Trace::new("info,term_transcript=debug");

fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/assets")
}

fn rainbow_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bin")
}

fn main_snapshot_path() -> PathBuf {
    assets_dir().join("rainbow.svg")
}

fn read_main_snapshot() -> io::Result<BufReader<File>> {
    File::open(main_snapshot_path()).map(BufReader::new)
}

fn read_pure_snapshot() -> io::Result<BufReader<File>> {
    File::open(assets_dir().join("rainbow-pure.svg")).map(BufReader::new)
}

fn read_custom_template() -> anyhow::Result<HandlebarsTemplate> {
    let template_string = fs::read_to_string(assets_dir().join("custom.html.handlebars"))?;
    HandlebarsTemplate::compile(&template_string).map_err(Into::into)
}

fn aliased_snapshot_path() -> &'static Path {
    Path::new("aliased.svg")
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn main_snapshot_can_be_rendered(pure_svg: bool) -> anyhow::Result<()> {
    let mut shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
    let mut transcript =
        Transcript::from_inputs(&mut shell_options, vec![UserInput::command("rainbow")])?;
    // Patch the exit status for cross-platform compatibility.
    transcript.interactions_mut()[0].set_exit_status(Some(ExitStatus(0)));

    let mut buffer = vec![];
    let template_options = TemplateOptions {
        palette: NamedPalette::Gjm8.into(),
        ..TemplateOptions::default()
    }
    .validated()?;
    let template = if pure_svg {
        Template::pure_svg(template_options)
    } else {
        Template::new(template_options)
    };
    template.render(&transcript, &mut buffer)?;
    let rendered = String::from_utf8(buffer)?;

    let mut snapshot = String::with_capacity(rendered.len());
    let mut snapshot_reader = if pure_svg {
        read_pure_snapshot()?
    } else {
        read_main_snapshot()?
    };
    snapshot_reader.read_to_string(&mut snapshot)?;

    // Normalize newlines.
    let rendered = rendered.replace("\r\n", "\n");
    let snapshot = snapshot.replace("\r\n", "\n");
    pretty_assertions::assert_eq!(rendered, snapshot);

    // Check that parsing SVG files doesn't lose information.
    let snapshot_reader = if pure_svg {
        read_pure_snapshot()?
    } else {
        read_main_snapshot()?
    };
    let parsed = Transcript::from_svg(snapshot_reader)?;
    let mut buffer = vec![];
    let stats = compare_transcripts(&mut buffer, &parsed, &transcript, MatchKind::Precise, false)?;
    if stats.errors(MatchKind::Precise) > 0 {
        panic!("{}", String::from_utf8_lossy(&buffer));
    }

    Ok(())
}

#[decorate(TRACING)]
#[test]
fn snapshot_with_custom_template() -> anyhow::Result<()> {
    let mut shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
    let transcript =
        Transcript::from_inputs(&mut shell_options, vec![UserInput::command("rainbow")])?;
    let template = read_custom_template()?;

    let template_options = TemplateOptions {
        palette: NamedPalette::Gjm8.into(),
        ..TemplateOptions::default()
    };
    let mut buffer = vec![];
    Template::custom(template, template_options.validated()?).render(&transcript, &mut buffer)?;
    let buffer = String::from_utf8(buffer)?;
    assert!(buffer.starts_with("<!DOCTYPE html>"), "{buffer}");
    Ok(())
}

#[cfg(feature = "portable-pty")]
#[test_casing(2, [false, true])]
#[test]
fn main_snapshot_can_be_rendered_from_pty(pure_svg: bool) -> anyhow::Result<()> {
    let mut shell_options =
        ShellOptions::new(PtyCommand::default()).with_additional_path(rainbow_dir());
    let transcript =
        Transcript::from_inputs(&mut shell_options, vec![UserInput::command("rainbow")])?;
    let template = if pure_svg {
        Template::pure_svg(ValidTemplateOptions::default())
    } else {
        Template::default()
    };
    template.render(&transcript, io::sink())?;
    Ok(())
}

#[cfg(feature = "portable-pty")]
#[test_casing(2, [false, true])]
#[test]
fn snapshot_with_long_lines_can_be_rendered_from_pty(pure_svg: bool) -> anyhow::Result<()> {
    let mut shell_options =
        ShellOptions::new(PtyCommand::default()).with_additional_path(rainbow_dir());
    let transcript = Transcript::from_inputs(
        &mut shell_options,
        vec![UserInput::command("rainbow --long-lines")],
    )?;

    let interaction = &transcript.interactions()[0];
    let output = interaction.output().to_plaintext()?;
    assert!(
        output.contains("\nblack red green yellow blue magenta cyan white"),
        "{output}"
    );

    let template = if pure_svg {
        Template::pure_svg(ValidTemplateOptions::default())
    } else {
        Template::default()
    };
    template.render(&transcript, io::sink())?;
    Ok(())
}

#[test]
fn snapshot_testing_low_level() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_main_snapshot()?)?;
    let shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn snapshot_testing(pure_svg: bool) {
    let shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
    let mut config = TestConfig::new(shell_options);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(main_snapshot_path(), ["rainbow"]);
}

#[cfg(feature = "portable-pty")]
#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn snapshot_testing_with_pty(pure_svg: bool) {
    let shell_options = ShellOptions::new(PtyCommand::default())
        .with_io_timeout(Duration::from_secs(2))
        .with_additional_path(rainbow_dir());
    let mut config = TestConfig::new(shell_options);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(main_snapshot_path(), ["rainbow"]);
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn animated_snapshot_testing(pure_svg: bool) {
    let shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
    let mut config = TestConfig::new(shell_options);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(
        assets_dir().join("animated.svg"),
        ["rainbow", "rainbow --long-lines"],
    );
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn snapshot_testing_with_custom_settings(pure_svg: bool) {
    let shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
    let mut config = TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(main_snapshot_path(), ["rainbow"]);
}

#[cfg(unix)]
#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn sh_shell_example(pure_svg: bool) {
    let rainbow_path = rainbow_dir().join("rainbow");
    let rainbow_path = rainbow_path.to_str().expect("non-UTF8 path");
    let shell_options = ShellOptions::sh().with_alias("colored-output", rainbow_path);
    let mut config = TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(aliased_snapshot_path(), ["colored-output"]);
}

// Although `bash` can be present on Windows, `with_alias` will most probably work
// improperly because of Windows-style paths.
#[cfg(unix)]
#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn bash_shell_example(pure_svg: bool) {
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

    let rainbow_path = rainbow_dir().join("rainbow");
    let rainbow_path = rainbow_path.to_str().expect("non-UTF8 path");
    let shell_options = ShellOptions::bash().with_alias("colored-output", rainbow_path);
    let mut config = TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(aliased_snapshot_path(), ["colored-output"]);
}

#[test_casing(2, [false, true])]
#[decorate(TRACING, Retry::times(3))] // PowerShell can be quite slow
fn powershell_example(pure_svg: bool) {
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

    let rainbow_path = rainbow_dir().join(if cfg!(windows) {
        "rainbow.bat"
    } else {
        "rainbow"
    });
    let rainbow_path = rainbow_path.to_str().expect("non-UTF8 path");
    let shell_options = ShellOptions::pwsh()
        .with_init_timeout(Duration::from_secs(2))
        .with_alias("colored-output", rainbow_path);
    let mut config = TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .with_output(TestOutputConfig::Verbose);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(aliased_snapshot_path(), ["colored-output"]);
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn repl_snapshot_testing(pure_svg: bool) {
    let shell_options = ShellOptions::from(Command::new(PATH_TO_REPL_BIN));
    let mut config = TestConfig::new(shell_options).with_match_kind(MatchKind::Precise);
    if pure_svg {
        config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
    }
    config.test(
        "repl.svg",
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
    const ALL: [Self; 3] = [
        Self::MissingSnapshot,
        Self::InputMismatch,
        Self::OutputMismatch,
    ];

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

#[test_casing(6, Product((ErrorType::ALL, [false, true])))]
fn new_snapshot(error_type: ErrorType, pure_svg: bool) -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let snapshot_path = temp_dir.path().join("rainbow.svg");
    error_type.create_snapshot(&snapshot_path)?;

    let test_result = panic::catch_unwind(|| {
        let shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
        let mut config = TestConfig::new(shell_options).with_update_mode(UpdateMode::Always);
        if pure_svg {
            config = config.with_template(Template::pure_svg(ValidTemplateOptions::default()));
        }
        config.test(&snapshot_path, ["rainbow"]);
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

#[test_casing(3, ErrorType::ALL)]
fn no_new_snapshot(error_type: ErrorType) -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let snapshot_path = temp_dir.path().join("rainbow.svg");
    error_type.create_snapshot(&snapshot_path)?;

    let test_result = panic::catch_unwind(|| {
        let shell_options = ShellOptions::default().with_additional_path(rainbow_dir());
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
