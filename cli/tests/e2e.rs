#![cfg(unix)]

use std::{fs::File, io, path::Path, time::Duration};

use term_transcript::{
    test::{MatchKind, TestConfig},
    ShellOptions, Transcript,
};

fn read_svg_snapshot(name: &str) -> io::Result<io::BufReader<File>> {
    let mut snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    snapshot_path.set_extension("svg");

    File::open(snapshot_path).map(io::BufReader::new)
}

#[cfg(feature = "portable-pty")]
#[test]
fn help_example() -> anyhow::Result<()> {
    use term_transcript::PtyCommand;

    let transcript = Transcript::from_svg(read_svg_snapshot("help")?)?;
    let shell_options = ShellOptions::new(PtyCommand::default())
        .with_io_timeout(Duration::from_millis(100))
        .with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test]
fn testing_example() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_svg_snapshot("test")?)?;
    let shell_options = ShellOptions::default()
        .with_io_timeout(Duration::from_millis(500))
        .with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}

#[test]
fn test_failure_example() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_svg_snapshot("test-fail")?)?;
    let shell_options = ShellOptions::default()
        .with_io_timeout(Duration::from_millis(500))
        .with_env("COLOR", "always")
        .with_cargo_path();
    TestConfig::new(shell_options)
        .with_match_kind(MatchKind::Precise)
        .test_transcript(&transcript);
    Ok(())
}
