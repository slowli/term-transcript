#![cfg(unix)]

use term_transcript::{read_svg_snapshot, test::TestConfig, ShellOptions, Transcript};

use std::time::Duration;

#[test]
fn testing_example() -> anyhow::Result<()> {
    let transcript = Transcript::from_svg(read_svg_snapshot!("test")?)?;
    let shell_options = ShellOptions::default()
        .with_io_timeout(Duration::from_millis(500))
        .with_current_dir("..")
        .with_cargo_path();
    TestConfig::new(shell_options).test_transcript(&transcript);
    Ok(())
}
