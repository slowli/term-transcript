#![cfg(unix)]

// TODO: use temporary dirs instead of just `/tmp`

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use term_transcript::{
    test::{MatchKind, TestConfig},
    ShellOptions,
};

fn svg_snapshot(name: &str) -> PathBuf {
    let mut snapshot_path = Path::new("tests/snapshots").join(name);
    snapshot_path.set_extension("svg");
    snapshot_path
}

fn test_config() -> TestConfig {
    let shell_options = ShellOptions::default()
        .with_env("COLOR", "always")
        .with_io_timeout(Duration::from_millis(500))
        .with_cargo_path();
    TestConfig::new(shell_options).with_match_kind(MatchKind::Precise)
}

#[cfg(feature = "portable-pty")]
#[test]
fn help_example() {
    use term_transcript::PtyCommand;

    let shell_options = ShellOptions::new(PtyCommand::default())
        .with_io_timeout(Duration::from_millis(100))
        .with_cargo_path();
    TestConfig::new(shell_options).test(svg_snapshot("help"), &["term-transcript --help"]);
}

#[test]
fn testing_example() {
    test_config().test(
        svg_snapshot("test"),
        &[
            "term-transcript exec -T 100 ./rainbow.sh > /tmp/rainbow.svg\n\
             # `-T` option defines the I/O timeout for the shell",
            "term-transcript test -T 100 -v /tmp/rainbow.svg\n\
             # `-v` switches on verbose output",
        ],
    );
}

#[test]
fn test_failure_example() {
    test_config().test(
        svg_snapshot("test-fail"),
        &[
            "term-transcript exec -T 100 './rainbow.sh --short' > /tmp/bogus.svg",
            "sed -i -E -e 's/(fg4|bg13)//g' /tmp/bogus.svg\n\
             # Mutate the captured output, removing one of the styles",
            "term-transcript test -T 100 --precise /tmp/bogus.svg\n\
             # --precise / -p flag enables comparison by style",
        ],
    );
}

#[test]
fn print_example() {
    test_config().test(
        svg_snapshot("print"),
        &[
            "term-transcript exec -T 100 './rainbow.sh --short' > /tmp/rainbow-short.svg",
            "term-transcript print /tmp/rainbow-short.svg",
        ],
    );
}
