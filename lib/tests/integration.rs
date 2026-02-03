//! Tests the full lifecycle of `Transcript`s.

use std::{
    cell::Cell,
    fmt, io,
    path::Path,
    process::{Command, Stdio},
    str::Utf8Error,
    time::Duration,
};

use assert_matches::assert_matches;
use term_transcript::{
    ShellOptions, Transcript, UserInput,
    svg::{Template, ValidTemplateOptions},
    test::{MatchKind, compare_transcripts},
};
use test_casing::{
    Product, decorate, decorators,
    decorators::{Retry, Trace},
    test_casing,
};
use tracing_capture::{CaptureLayer, CapturedSpan, SharedStorage, Storage};
use tracing_subscriber::layer::SubscriberExt;

static TRACING: Trace = Trace::new("info,term_transcript=debug");

#[derive(Debug)]
struct TracingWithStorage;

thread_local! {
    static TRACING_STORAGE: Cell<Option<SharedStorage>> = Cell::default();
}

impl TracingWithStorage {
    fn take_storage() -> SharedStorage {
        TRACING_STORAGE.take().expect("no injected storage")
    }
}

impl<R> decorators::DecorateTest<R> for TracingWithStorage {
    fn decorate_and_test<F: decorators::TestFn<R>>(&'static self, test_fn: F) -> R {
        let storage = SharedStorage::default();
        let subscriber = TRACING
            .create_subscriber()
            .with(CaptureLayer::new(&storage));
        let _guard = tracing::subscriber::set_default(subscriber);
        TRACING_STORAGE.set(Some(storage));
        test_fn()
    }
}

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

#[test_casing(2, [false, true])]
#[decorate(TracingWithStorage)]
#[test]
fn transcript_lifecycle(pure_svg: bool) -> anyhow::Result<()> {
    let tracing_storage = TracingWithStorage::take_storage();
    let mut transcript = Transcript::new();

    // 1. Capture output from a command.
    transcript.capture_output(
        UserInput::command("echo \"Hello, world!\""),
        &mut echo_command(),
    )?;
    assert_tracing_for_output_capture(&tracing_storage.lock());

    // 2. Render the transcript into SVG.
    let mut svg_buffer = vec![];
    let options = ValidTemplateOptions::default();
    let template = if pure_svg {
        Template::pure_svg(options)
    } else {
        Template::new(options)
    };
    template.render(&transcript, &mut svg_buffer)?;

    // 3. Parse SVG back to the transcript.
    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    assert_eq!(parsed.interactions().len(), 1);
    let interaction = &parsed.interactions()[0];
    assert_eq!(
        *interaction.input(),
        UserInput::command("echo \"Hello, world!\"")
    );
    assert_tracing_for_parsing(&tracing_storage.lock());

    // 4. Compare output to the output in the original transcript.
    assert_eq!(
        interaction.output().plaintext(),
        transcript.interactions()[0].output().to_plaintext()?
    );
    Ok(())
}

fn assert_tracing_for_output_capture(storage: &Storage) {
    let span = storage
        .all_spans()
        .find(|span| span.metadata().name() == "capture_output")
        .expect("`capture_output` span not found");
    assert!(span["command"].as_debug_str().is_some());
    assert_eq!(
        span["input.text"].as_debug_str(),
        Some(r#"echo "Hello, world!""#)
    );

    let output_event = span
        .events()
        .find(|event| event.message() == Some("read command output"))
        .expect("no output event");
    let output = output_event["output"].as_debug_str().unwrap();
    assert!(output.starts_with(r#""Hello, world"#));
    // ^ The output may have `\r\n` or `\n` ending depending on the OS, so we don't check it.
}

fn assert_tracing_for_parsing(storage: &Storage) {
    let span = storage
        .all_spans()
        .find(|span| span.metadata().name() == "from_svg")
        .expect("`from_svg` span not found");

    let interaction_event = span
        .events()
        .find(|event| event.message() == Some("parsed interaction"))
        .expect("new interaction event not found");
    assert!(
        interaction_event["interaction.input"]
            .is_debug(&UserInput::command(r#"echo "Hello, world!""#))
    );
    let output = interaction_event["interaction.output"]
        .as_debug_str()
        .unwrap();
    assert!(output.starts_with("\"Hello, world!"), "{output}");
}

const MUTE_OUTPUT_CASES: [&[bool]; 6] = [
    &[true],
    &[true, false],
    &[false, true],
    &[false, true, false],
    &[true, false, true],
    &[true, true, false, true],
];

#[test_casing(12, Product((MUTE_OUTPUT_CASES, [false, true])))]
#[decorate(TracingWithStorage)]
fn transcript_with_empty_output(mute_outputs: &[bool], pure_svg: bool) -> anyhow::Result<()> {
    #[cfg(unix)]
    const NULL_FILE: &str = "/dev/null";
    #[cfg(windows)]
    const NULL_FILE: &str = "NUL";

    let tracing_storage = TracingWithStorage::take_storage();
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
    assert_tracing_for_transcript_from_inputs(&tracing_storage.lock());

    let mut svg_buffer = vec![];
    let template = if pure_svg {
        Template::pure_svg(ValidTemplateOptions::default())
    } else {
        Template::default()
    };
    template.render(&transcript, &mut svg_buffer)?;
    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;

    assert_eq!(parsed.interactions().len(), mute_outputs.len());

    for (interaction, &mute) in parsed.interactions().iter().zip(mute_outputs) {
        if mute {
            assert_eq!(interaction.output().plaintext(), "");
        } else {
            assert_ne!(interaction.output().plaintext(), "");
        }
    }
    Ok(())
}

fn assert_tracing_for_transcript_from_inputs(storage: &Storage) {
    let root_span = storage
        .all_spans()
        .find(|span| span.metadata().name() == "from_inputs")
        .expect("`from_inputs` span not found");
    assert!(root_span["options.io_timeout"].is_debug(&Duration::from_millis(200)));

    let spawn_shell_span = root_span
        .children()
        .find(|span| span.metadata().name() == "spawn_shell")
        .expect("`spawn_shell` span not found");
    let path_additions = spawn_shell_span["self.path_additions"]
        .as_debug_str()
        .unwrap();
    assert!(
        path_additions.starts_with('[') && path_additions.ends_with(']'),
        "{path_additions:?}"
    );

    root_span
        .children()
        .find(|span| span.metadata().name() == "push_init_commands")
        .expect("`push_init_commands` span not found");
    root_span
        .children()
        .find(|span| span.metadata().name() == "record_interaction")
        .expect("`record_interaction` spans not found");

    let written_lines = root_span.descendants().filter_map(|span| {
        if span.metadata().name() == "write_line" {
            span["line"].as_debug_str().map(str::to_owned)
        } else {
            None
        }
    });
    let written_lines: Vec<_> = written_lines.collect();
    assert!(
        written_lines
            .iter()
            .all(|line| line.starts_with("echo \"Hello, world!\"")),
        "{written_lines:?}"
    );
}

#[cfg(unix)]
#[decorate(TRACING)]
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
#[decorate(TracingWithStorage, Retry::times(3))] // PowerShell can be quite slow
fn command_exit_status_in_powershell() -> anyhow::Result<()> {
    fn powershell_exists() -> bool {
        let exit_status = Command::new("pwsh")
            .arg("-Help")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        matches!(exit_status, Ok(status) if status.success())
    }

    let tracing_storage = TracingWithStorage::take_storage();
    if !powershell_exists() {
        println!("pwsh not found; exiting");
        return Ok(());
    }

    let mut options = ShellOptions::pwsh()
        .with_init_command("echo \"Hello world!\"")
        // ^ The first command executed by `pwsh` can take really long, so we warm up.
        .with_init_timeout(Duration::from_secs(3))
        .with_io_timeout(Duration::from_secs(1))
        .with_lossy_utf8_decoder();
    // ^ The error output is locale-specific and is not always UTF-8
    let inputs = [
        UserInput::command("echo \"Hello world!\""),
        UserInput::command("cargo what"),
    ];
    let transcript = Transcript::from_inputs(&mut options, inputs)?;

    let exit_status = transcript.interactions()[0].exit_status().unwrap();
    assert!(exit_status.is_success(), "{exit_status:?}");
    let exit_status = transcript.interactions()[1].exit_status().unwrap();
    assert!(!exit_status.is_success(), "{exit_status:?}");

    assert_tracing_for_powershell(&tracing_storage.lock());
    Ok(())
}

fn assert_tracing_for_powershell(storage: &Storage) {
    let echo_spans: Vec<_> = storage
        .all_spans()
        .filter(|span| span.metadata().name() == "read_echo")
        .collect();

    assert!(
        echo_spans
            .iter()
            .any(|span| span["input_line"].as_str() == Some("cargo what"))
    );

    let received_line_events: Vec<_> = echo_spans
        .iter()
        .flat_map(CapturedSpan::events)
        .filter(|event| event.message() == Some("received line"))
        .collect();
    assert_eq!(received_line_events.len(), echo_spans.len());
    for event in &received_line_events {
        assert!(event["line_utf8"].as_str().is_some());
    }
}

/// The default `cmd` codepage can lead to non-UTF8 output for builtin commands
/// (e.g., `dir` may output non-breakable space in file sizes as 0xff).
/// Here, we test that the codepage is switched to UTF-8.
#[cfg(windows)]
#[decorate(TRACING)]
#[test]
fn cmd_shell_with_non_utf8_output() {
    let input = UserInput::command(format!("dir {}", env!("CARGO_MANIFEST_DIR")));
    let transcript = Transcript::from_inputs(&mut ShellOptions::default(), vec![input]).unwrap();

    assert_eq!(transcript.interactions().len(), 1);
    let output = transcript.interactions()[0].output().as_ref();
    assert!(output.contains("LICENSE-APACHE"));
    assert!(!output.contains('\r'));
}

#[cfg(all(windows, feature = "portable-pty"))]
#[decorate(TRACING)]
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
    Template::default()
        .render(&transcript, &mut vec![])
        .unwrap();
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn non_utf8_shell_output(lossy: bool) -> anyhow::Result<()> {
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

    let mut options = ShellOptions::default();
    if lossy {
        options = options.with_lossy_utf8_decoder();
    }

    let result = Transcript::from_inputs(&mut options, vec![input]);
    if lossy {
        let transcript = result.unwrap();
        let output = transcript.interactions()[0].output();
        assert!(output.to_plaintext()?.contains(char::REPLACEMENT_CHARACTER));
    } else {
        let err = result.unwrap_err();
        assert_matches!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.get_ref().unwrap().is::<Utf8Error>(), "{err:?}");
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct TestOutput {
    name: &'static str,
    content: &'static str,
}

impl fmt::Debug for TestOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.name, formatter)
    }
}

macro_rules! test_output {
    ($path:tt) => {
        TestOutput {
            name: $path,
            content: ::core::include_str!($path),
        }
    };
}

const RAINBOW_OUTPUTS: [TestOutput; 3] = [
    test_output!("outputs/rainbow.out"),
    test_output!("outputs/rainbow-short.out"),
    test_output!("outputs/rainbow-long.out"),
];

#[test_casing(6, Product((RAINBOW_OUTPUTS, [false, true])))]
#[decorate(TRACING)]
fn transcript_roundtrip_for_rainbow_outputs(
    output: TestOutput,
    pure_svg: bool,
) -> anyhow::Result<()> {
    // 1. Prepare a transcript from the predefined output.
    let mut transcript = Transcript::new();
    transcript.add_interaction(output.name, output.content);

    // 2. Render the transcript into SVG.
    let mut svg_buffer = vec![];
    let options = ValidTemplateOptions::default();
    let template = if pure_svg {
        Template::pure_svg(options)
    } else {
        Template::new(options)
    };
    template.render(&transcript, &mut svg_buffer)?;

    // 3. Parse SVG back to the transcript.
    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    assert_eq!(parsed.interactions().len(), 1);
    let interaction = &parsed.interactions()[0];
    assert_eq!(*interaction.input(), UserInput::command(output.name));

    // 4. Compare output to the output in the original transcript.
    let mut buffer = vec![];
    let stats = compare_transcripts(&mut buffer, &parsed, &transcript, MatchKind::Precise, false)?;
    assert_eq!(
        stats.errors(MatchKind::Precise),
        0,
        "{}",
        String::from_utf8_lossy(&buffer)
    );
    Ok(())
}

#[test_casing(2, [false, true])]
#[decorate(TRACING)]
fn transcript_roundtrip_with_empty_lines(pure_svg: bool) -> anyhow::Result<()> {
    let test_output = TestOutput {
        name: "(with-empty-lines)",
        content: "test\n\ntest",
    };
    transcript_roundtrip_for_rainbow_outputs(test_output, pure_svg)
}
