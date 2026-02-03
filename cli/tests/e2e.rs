//! End-to-end CLI tests. Includes anchors for including file portions to the Book.

#![cfg(unix)]

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tempfile::{tempdir, TempDir};
use term_transcript::{
    svg::{ScrollOptions, Template, TemplateOptions, WindowOptions},
    test::{MatchKind, TestConfig},
    ShellOptions, StdShell,
};

// ANCHOR: snapshots_path
fn svg_snapshot(name: &str) -> PathBuf {
    let mut snapshot_path = Path::new("tests/snapshots").join(name);
    snapshot_path.set_extension("svg");
    snapshot_path
}
// ANCHOR_END: snapshots_path

// ANCHOR: config
// Executes commands in a temporary dir, with paths to the `term-transcript` binary and
// the `rainbow` script added to PATH.
fn test_config() -> (TestConfig<StdShell>, TempDir) {
    let temp_dir = tempdir().expect("cannot create temporary directory");
    let rainbow_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e-tests/rainbow/bin");

    let shell_options = ShellOptions::sh()
        .with_env("COLOR", "always")
        // Switch off logging if `RUST_LOG` is set in the surrounding env
        .with_env("RUST_LOG", "off")
        .with_current_dir(temp_dir.path())
        .with_cargo_path()
        .with_additional_path(rainbow_dir)
        .with_io_timeout(Duration::from_secs(2));
    let config = TestConfig::new(shell_options).with_match_kind(MatchKind::Precise);
    (config, temp_dir)
}
// ANCHOR_END: config

// ANCHOR: template
fn scrolled_template() -> Template {
    let template_options = TemplateOptions {
        window: Some(WindowOptions::default()),
        scroll: Some(ScrollOptions::default()),
        ..TemplateOptions::default()
    };
    Template::new(template_options.validated().unwrap())
}
// ANCHOR_END: template

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
            "term-transcript exec -I 300ms -T 100ms rainbow > rainbow.svg\n\
             # `-T` option defines the I/O timeout for the shell,\n\
             # and `-I` specifies the additional initialization timeout",
            "term-transcript test -I 300ms -T 100ms -v rainbow.svg\n\
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
            "term-transcript exec -I 300ms -T 100ms 'rainbow --short' > bogus.svg && \\\n  \
             sed -i~ -E -e 's/(fg4|bg13)//g' bogus.svg\n\
             # Mutate the captured output, removing some styles",
            "term-transcript test -I 300ms -T 100ms --precise bogus.svg\n\
             # --precise / -p flag enables comparison by style",
        ],
    );
}

// ANCHOR: simple_test
#[test]
fn print_example() {
    let (mut config, _dir) = test_config();
    config.test(
        svg_snapshot("print"),
        [
            "term-transcript exec -I 300ms -T 100ms 'rainbow --short' > short.svg",
            "term-transcript print short.svg",
        ],
    );
}
// ANCHOR_END: simple_test

#[test]
fn print_example_with_failures() {
    let (mut config, _dir) = test_config();
    config.test(
        svg_snapshot("print-with-failures"),
        [
            "term-transcript exec -I 300ms -T 100ms \\\n  \
             'which some-non-existing-command > /dev/null' \\\n  \
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
            "rainbow | term-transcript capture 'rainbow' > captured.svg",
            "term-transcript print captured.svg",
        ],
    );
}
