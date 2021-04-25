#![cfg(unix)]

use term_transcript::{test::TestConfig, ShellOptions, Transcript};

use std::{fs::File, io, path::Path, time::Duration};

fn read_svg_snapshot(name: &str) -> io::Result<io::BufReader<File>> {
    let mut snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    snapshot_path.set_extension("svg");

    File::open(snapshot_path).map(io::BufReader::new)
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