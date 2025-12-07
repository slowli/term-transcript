use termcolor::NoColor;
use test_casing::test_casing;

use super::{color_diff::ColorSpan, *};
use crate::{
    svg::{Template, TemplateOptions},
    Captured, Interaction, Transcript, UserInput,
};

#[test_casing(2, [MatchKind::TextOnly, MatchKind::Precise])]
fn snapshot_testing(match_kind: MatchKind) -> anyhow::Result<()> {
    let mut test_config = TestConfig::new(ShellOptions::default()).with_match_kind(match_kind);
    let transcript = Transcript::from_inputs(
        &mut ShellOptions::default(),
        vec![UserInput::command("echo \"Hello, world!\"")],
    )?;

    let mut svg_buffer = vec![];
    Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    test_config.test_transcript(&parsed);
    Ok(())
}

fn test_negative_snapshot_testing(
    out: &mut Vec<u8>,
    test_config: &mut TestConfig,
) -> anyhow::Result<()> {
    let mut transcript = Transcript::from_inputs(
        &mut ShellOptions::default(),
        vec![UserInput::command("echo \"Hello, world!\"")],
    )?;
    transcript.add_interaction(UserInput::command("echo \"Sup?\""), "Nah");

    let mut svg_buffer = vec![];
    Template::new(TemplateOptions::default()).render(&transcript, &mut svg_buffer)?;

    let parsed = Transcript::from_svg(svg_buffer.as_slice())?;
    let (stats, _) = test_config.test_transcript_inner(&mut NoColor::new(out), &parsed)?;
    assert_eq!(stats.errors(MatchKind::TextOnly), 1);
    Ok(())
}

#[test]
fn negative_snapshot_testing_with_default_output() {
    let mut out = vec![];
    let mut test_config =
        TestConfig::new(ShellOptions::default()).with_color_choice(ColorChoice::Never);
    test_negative_snapshot_testing(&mut out, &mut test_config).unwrap();

    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("[+] Input: echo \"Hello, world!\""), "{out}");
    assert_eq!(out.matches("Hello, world!").count(), 1, "{out}");
    // ^ output for successful interactions should not be included
    assert!(out.contains("[-] Input: echo \"Sup?\""), "{out}");
    assert!(out.contains("Nah"), "{out}");
}

#[test]
fn negative_snapshot_testing_with_verbose_output() {
    let mut out = vec![];
    let mut test_config = TestConfig::new(ShellOptions::default())
        .with_output(TestOutputConfig::Verbose)
        .with_color_choice(ColorChoice::Never);
    test_negative_snapshot_testing(&mut out, &mut test_config).unwrap();

    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("[+] Input: echo \"Hello, world!\""), "{out}");
    assert_eq!(out.matches("Hello, world!").count(), 2, "{out}");
    // ^ output for successful interactions should be included
    assert!(out.contains("[-] Input: echo \"Sup?\""), "{out}");
    assert!(out.contains("Nah"), "{out}");
}

fn diff_snapshot_with_color(expected_capture: &str, actual_capture: &str) -> (TestStats, String) {
    let expected_capture = Captured::from(expected_capture.to_owned());
    let parsed = Transcript {
        interactions: vec![Interaction {
            input: UserInput::command("test"),
            output: Parsed {
                plaintext: expected_capture.to_plaintext().unwrap(),
                color_spans: ColorSpan::parse(expected_capture.as_ref()).unwrap(),
            },
            exit_status: None,
        }],
    };

    let mut reproduced = Transcript::new();
    reproduced.add_interaction(UserInput::command("test"), actual_capture);

    let mut out: Vec<u8> = vec![];
    let stats = compare_transcripts(
        &mut NoColor::new(&mut out),
        &parsed,
        &reproduced,
        MatchKind::Precise,
        false,
    )
    .unwrap();
    (stats, String::from_utf8(out).unwrap())
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
    assert!(out.contains("13..14 ----   yellow/(none)   ----     blue/(none)"));
}
