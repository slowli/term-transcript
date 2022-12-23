#![cfg(unix)]

use tempfile::{tempdir, TempDir};

use std::path::{Path, PathBuf};

use term_transcript::{
    svg::{ScrollOptions, Template, TemplateOptions},
    test::{MatchKind, TestConfig},
    ShellOptions, StdShell,
};

fn svg_snapshot(name: &str) -> PathBuf {
    let mut snapshot_path = Path::new("tests/snapshots").join(name);
    snapshot_path.set_extension("svg");
    snapshot_path
}

// Executes commands in a temporary dir, with paths to the `term-transcript` binary and
// the `rainbow.sh` example added to PATH.
fn test_config() -> (TestConfig<StdShell>, TempDir) {
    let temp_dir = tempdir().expect("cannot create temporary directory");
    let rainbow_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let shell_options = ShellOptions::sh()
        .with_env("COLOR", "always")
        .with_current_dir(temp_dir.path())
        .with_cargo_path()
        .with_additional_path(rainbow_dir);
    let config = TestConfig::new(shell_options).with_match_kind(MatchKind::Precise);
    (config, temp_dir)
}

fn scrolled_template() -> Template {
    let template_options = TemplateOptions {
        window_frame: true,
        scroll: Some(ScrollOptions::default()),
        ..TemplateOptions::default()
    };
    Template::new(template_options)
}

#[cfg(feature = "portable-pty")]
#[test]
fn help_example() {
    use term_transcript::PtyCommand;

    let shell_options = ShellOptions::new(PtyCommand::default()).with_cargo_path();
    TestConfig::new(shell_options).test(svg_snapshot("help"), ["term-transcript --help"]);
}

#[test]
fn testing_example() {
    let (config, _dir) = test_config();
    config.with_template(scrolled_template()).test(
        svg_snapshot("test"),
        [
            "term-transcript exec -T 200 rainbow.sh > rainbow.svg\n\
             # `-T` option defines the I/O timeout for the shell",
            "term-transcript test -T 200 -v rainbow.svg\n\
             # `-v` switches on verbose output",
        ],
    );
}

#[test]
fn test_failure_example() {
    let (mut config, _dir) = test_config();
    config.test(
        svg_snapshot("test-fail"),
        [
            "term-transcript exec -T 200 'rainbow.sh --short' > bogus.svg && \\\n  \
             sed -i -E -e 's/(fg4|bg13)//g' bogus.svg\n\
             # Mutate the captured output, removing some styles",
            "term-transcript test -T 200 --precise bogus.svg\n\
             # --precise / -p flag enables comparison by style",
        ],
    );
}

#[test]
fn print_example() {
    let (mut config, _dir) = test_config();
    config.test(
        svg_snapshot("print"),
        [
            "term-transcript exec -T 200 'rainbow.sh --short' > short.svg",
            "term-transcript print short.svg",
        ],
    );
}

#[test]
fn print_example_with_failures() {
    let (mut config, _dir) = test_config();
    config.test(
        svg_snapshot("print-with-failures"),
        [
            "term-transcript exec -T 200 'some-non-existing-command' \\\n  \
             '[ -x some-non-existing-file ]' > fail.svg",
            "term-transcript print fail.svg",
        ],
    );
}

#[test]
fn capture_example() {
    let (config, _dir) = test_config();
    config.with_template(scrolled_template()).test(
        svg_snapshot("capture"),
        [
            "rainbow.sh | term-transcript capture 'rainbow.sh' > captured.svg",
            "term-transcript print captured.svg",
        ],
    );
}
