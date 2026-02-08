use anstream::StripStream;
use term_style::{StyledString, styled};
use test_casing::test_casing;

use super::*;
use crate::{Transcript, UserInput, svg::Template};

#[test_casing(2, [MatchKind::TextOnly, MatchKind::Precise])]
fn snapshot_testing(match_kind: MatchKind) -> anyhow::Result<()> {
    let mut test_config = TestConfig::new(ShellOptions::default()).with_match_kind(match_kind);
    let transcript = Transcript::from_inputs(
        &mut ShellOptions::default(),
        vec![UserInput::command("echo \"Hello, world!\"")],
    )?;

    let mut svg_buffer = vec![];
    Template::default().render(&transcript, &mut svg_buffer)?;

    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    test_config.test_transcript(&parsed);
    Ok(())
}

fn test_negative_snapshot_testing(test_config: &mut TestConfig) -> anyhow::Result<String> {
    let mut transcript = Transcript::from_inputs(
        &mut ShellOptions::default(),
        vec![UserInput::command("echo \"Hello, world!\"")],
    )?;
    transcript.add_interaction(UserInput::command("echo \"Sup?\""), styled!("Nah").into());

    let mut svg_buffer = vec![];
    Template::default().render(&transcript, &mut svg_buffer)?;

    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    let mut out = StripStream::new(vec![]);
    let (stats, _) = test_config.test_transcript_inner(&mut out, &parsed)?;
    assert_eq!(stats.errors(MatchKind::TextOnly), 1);
    String::from_utf8(out.into_inner()).map_err(Into::into)
}

#[test]
fn negative_snapshot_testing_with_default_output() {
    let mut test_config =
        TestConfig::new(ShellOptions::default()).with_color_choice(ColorChoice::Never);
    let out = test_negative_snapshot_testing(&mut test_config).unwrap();

    assert!(out.contains("[+] Input: echo \"Hello, world!\""), "{out}");
    assert_eq!(out.matches("Hello, world!").count(), 1, "{out}");
    // ^ output for successful interactions should not be included
    assert!(out.contains("[-] Input: echo \"Sup?\""), "{out}");
    assert!(out.contains("Nah"), "{out}");
}

#[test]
fn negative_snapshot_testing_with_verbose_output() {
    let mut test_config = TestConfig::new(ShellOptions::default())
        .with_output(TestOutputConfig::Verbose)
        .with_color_choice(ColorChoice::Never);
    let out = test_negative_snapshot_testing(&mut test_config).unwrap();

    assert!(out.contains("[+] Input: echo \"Hello, world!\""), "{out}");
    assert_eq!(out.matches("Hello, world!").count(), 2, "{out}");
    // ^ output for successful interactions should be included
    assert!(out.contains("[-] Input: echo \"Sup?\""), "{out}");
    assert!(out.contains("Nah"), "{out}");
}

fn diff_snapshot_with_color(expected_capture: &str, actual_capture: &str) -> (TestStats, String) {
    let mut parsed = Transcript::new();
    parsed.add_interaction(
        UserInput::command("test"),
        StyledString::from_ansi(expected_capture).unwrap(),
    );

    let mut reproduced = Transcript::new();
    reproduced.add_interaction(
        UserInput::command("test"),
        StyledString::from_ansi(actual_capture).unwrap(),
    );

    let mut out = StripStream::new(vec![]);
    let stats =
        compare_transcripts(&mut out, &parsed, &reproduced, MatchKind::Precise, false).unwrap();
    let out = String::from_utf8(out.into_inner()).unwrap();
    (stats, out)
}

#[test]
fn snapshot_testing_with_color_diff() {
    let (stats, out) = diff_snapshot_with_color(
        "Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m",
        "Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m",
    );

    assert_eq!(stats.matches(), [Some(MatchKind::Precise)]);
    assert!(out.contains("[+] Input: test"), "{out}");
}

#[test]
fn no_match_for_snapshot_testing_with_color_diff() {
    let (stats, out) = diff_snapshot_with_color(
        "Apr 18 12:54 \u{1b}[0m\u{1b}[33m.\u{1b}[0m",
        "Apr 19 12:54 \u{1b}[0m\u{1b}[33m.\u{1b}[0m",
    );

    assert_eq!(stats.matches(), [None]);
    assert!(out.contains("[-] Input: test"), "{out}");
}

#[test]
fn text_match_for_snapshot_testing_with_color_diff() {
    let (stats, out) = diff_snapshot_with_color(
        "Apr 18 12:54 \u{1b}[0m\u{1b}[33m.\u{1b}[0m",
        "Apr 18 12:54 \u{1b}[0m\u{1b}[34m.\u{1b}[0m",
    );

    assert_eq!(stats.matches(), [Some(MatchKind::TextOnly)]);
    assert!(out.contains("[#] Input: test"), "{out}");
    assert!(
        out.contains("    13..14          yellow                     blue           "),
        "{out}"
    );
}
