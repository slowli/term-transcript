//! Tests for the SVG rendering logic.

use super::*;
use crate::{ExitStatus, Interaction, UserInput};

#[test]
fn rendering_simple_transcript() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    Template::new(TemplateOptions::default())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.starts_with("<!--"));
    assert!(
        buffer.ends_with("</svg>\n") || buffer.ends_with("</svg>\r\n"),
        // ^-- allows for different newline chars in Windows
        "unexpected rendering result: {buffer}"
    );
    assert!(buffer.contains(r#"Hello, <span class="fg2">world</span>!"#));
    assert!(!buffer.contains("data-exit-status"));
    assert!(!buffer.contains("<circle"));

    assert!(!buffer.contains("input-failure"));
    assert!(!buffer.contains("title=\"This command exited with non-zero code\""));
}

#[test]
fn rendering_simple_transcript_to_pure_svg() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("test --arg"),
        "Hello, \u{1b}[31m\u{1b}[42mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    Template::pure_svg(TemplateOptions::default())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let top_svg = "<svg viewBox=\"0 0 720 118\"";
    assert!(buffer.contains(top_svg), "{buffer}");
    let first_input_text = r#"<tspan xml:space="preserve" x="10" y="16" class="input">"#;
    assert!(buffer.contains(first_input_text), "{buffer}");
    let first_output_text = r#"<tspan xml:space="preserve" x="10" y="42" class="output">"#;
    assert!(buffer.contains(first_output_text), "{buffer}");
    let second_input_text = r#"<tspan xml:space="preserve" x="10" y="68" class="input">"#;
    assert!(buffer.contains(second_input_text), "{buffer}");
    let second_output_text = r#"<tspan xml:space="preserve" x="10" y="94" class="output">"#;
    assert!(buffer.contains(second_output_text), "{buffer}");
}

#[test]
fn rendering_transcript_with_empty_output_to_pure_svg() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(UserInput::command("test"), "");
    transcript.add_interaction(
        UserInput::command("test --arg"),
        "Hello, \u{1b}[31m\u{1b}[42mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    Template::pure_svg(TemplateOptions::default())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let top_svg = "<svg viewBox=\"0 0 720 94\"";
    assert!(buffer.contains(top_svg), "{buffer}");
    let second_input_bg = r#"<rect x="0" y="28" width="100%" height="22""#;
    assert!(buffer.contains(second_input_bg), "{buffer}");
    let second_input_text = r#"<tspan xml:space="preserve" x="10" y="44" class="input">"#;
    assert!(buffer.contains(second_input_text), "{buffer}");
    let second_output_bg = r#"<tspan xml:space="preserve" x="10" y="70" class="output-bg">"#;
    assert!(buffer.contains(second_output_bg), "{buffer}");
}

#[test]
fn rendering_transcript_with_explicit_success() {
    let mut transcript = Transcript::new();
    let interaction = Interaction::new("test", "Hello, \u{1b}[32mworld\u{1b}[0m!")
        .with_exit_status(ExitStatus(0));
    transcript.add_existing_interaction(interaction);

    let mut buffer = vec![];
    Template::new(TemplateOptions::default())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(!buffer.contains("input-failure"));
    assert!(!buffer.contains("title=\"This command exited with non-zero code\""));
    assert!(buffer.contains(r#"data-exit-status="0""#));
}

#[test]
fn rendering_transcript_with_failure() {
    let mut transcript = Transcript::new();
    let interaction = Interaction::new("test", "Hello, \u{1b}[32mworld\u{1b}[0m!")
        .with_exit_status(ExitStatus(1));
    transcript.add_existing_interaction(interaction);

    let mut buffer = vec![];
    Template::new(TemplateOptions::default())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains("input-failure"));
    assert!(buffer.contains("title=\"This command exited with non-zero code\""));
    assert!(buffer.contains(r#"data-exit-status="1""#));
}

#[test]
fn rendering_pure_svg_transcript_with_failure() {
    let mut transcript = Transcript::new();
    let interaction = Interaction::new("test", "Hello, \u{1b}[32mworld\u{1b}[0m!")
        .with_exit_status(ExitStatus(1));
    transcript.add_existing_interaction(interaction);

    let mut buffer = vec![];
    Template::pure_svg(TemplateOptions::default())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(".input-bg .input-failure"), "{buffer}");
    let failure_bg = "<rect x=\"0\" y=\"0\" width=\"100%\" height=\"22\" \
            class=\"input-failure\"";
    assert!(buffer.contains(failure_bg), "{buffer}");
    let left_failure_border = "<rect x=\"0\" y=\"0\" width=\"2\" height=\"22\" \
            class=\"input-failure-hl\" />";
    assert!(buffer.contains(left_failure_border), "{buffer}");
    let right_failure_border = "<rect x=\"100%\" y=\"0\" width=\"2\" height=\"22\" \
            class=\"input-failure-hl\" transform=\"translate(-2, 0)\" />";
    assert!(buffer.contains(right_failure_border), "{buffer}");
    assert!(buffer.contains("<title>This command exited"), "{buffer}");
}

#[test]
fn rendering_transcript_with_frame() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        window_frame: true,
        ..TemplateOptions::default()
    };
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();
    assert!(buffer.contains("<circle"));
}

#[test]
fn rendering_pure_svg_transcript_with_frame() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        window_frame: true,
        ..TemplateOptions::default()
    };
    Template::pure_svg(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();
    assert!(buffer.contains("<circle"));
}

#[test]
fn rendering_transcript_with_animation() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!\n".repeat(22),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        scroll: Some(ScrollOptions {
            max_height: 240,
            interval: 3.0,
        }),
        ..TemplateOptions::default()
    };
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 260""#), "{buffer}");
    assert!(buffer.contains("<animateTransform"), "{buffer}");
}

#[test]
fn rendering_pure_svg_transcript_with_animation() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!\n".repeat(22),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        scroll: Some(ScrollOptions {
            max_height: 240,
            interval: 3.0,
        }),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 260""#), "{buffer}");
    assert!(buffer.contains("<animateTransform"), "{buffer}");
}

#[test]
fn rendering_transcript_with_wraps() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        wrap: Some(WrapOptions::HardBreakAt(5)),
        ..TemplateOptions::default()
    };
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{buffer}");
    assert!(buffer.contains("<br/>"), "{buffer}");
}

#[test]
fn rendering_svg_transcript_with_wraps() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        wrap: Some(WrapOptions::HardBreakAt(5)),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{buffer}");
    assert!(buffer.contains("Hello<tspan class=\"hard-br\""), "{buffer}");
}

#[test]
fn rendering_transcript_with_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another_test"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumbers::EachOutput),
        ..TemplateOptions::default()
    };
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(
        buffer.contains(r#"<pre class="line-numbers">1</pre>"#),
        "{buffer}"
    );
    assert!(
        buffer.contains(r#"<pre class="line-numbers">1<br/>2</pre>"#),
        "{buffer}"
    );
}

#[test]
fn rendering_pure_svg_transcript_with_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another_test"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumbers::EachOutput),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(".line-numbers {"), "{buffer}");
    let first_output_ln = r#"<tspan x="34" y="42">1</tspan>"#;
    assert!(buffer.contains(first_output_ln), "{buffer}");
    let second_output_ln1 = r#"<tspan x="34" y="94">1</tspan>"#;
    assert!(buffer.contains(second_output_ln1), "{buffer}");
    let second_output_ln2 = r#"<tspan x="34" y="112">2</tspan>"#;
    assert!(buffer.contains(second_output_ln2), "{buffer}");
}

#[test]
fn rendering_transcript_with_continuous_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another_test"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumbers::ContinuousOutputs),
        ..TemplateOptions::default()
    };
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(
        buffer.contains(r#"<pre class="line-numbers">1</pre>"#),
        "{buffer}"
    );
    assert!(
        buffer.contains(r#"<pre class="line-numbers">2<br/>3</pre>"#),
        "{buffer}"
    );
}

#[test]
fn rendering_transcript_with_input_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumbers::Continuous),
        ..TemplateOptions::default()
    };
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(
        buffer.contains(r#"<div class="input"><pre class="line-numbers">"#),
        "{buffer}"
    );
    assert!(
        buffer.contains(r#"<pre class="line-numbers">5<br/>6</pre>"#),
        "{buffer}"
    );
}

#[test]
fn rendering_pure_svg_transcript_with_input_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumbers::Continuous),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let line_numbers = "<tspan x=\"34\" y=\"16\">1</tspan>\
            <tspan x=\"34\" y=\"42\">2</tspan>\
            <tspan x=\"34\" y=\"68\">3</tspan>\
            <tspan x=\"34\" y=\"86\">4</tspan>\
            <tspan x=\"34\" y=\"112\">5</tspan>\
            <tspan x=\"34\" y=\"130\">6</tspan>";
    assert!(buffer.contains(line_numbers), "{buffer}");
}

#[test]
fn rendering_transcript_with_styles() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let styles = "@font-face { font-family: 'Fira Mono'; }";
    let options = TemplateOptions {
        additional_styles: styles.to_owned(),
        font_family: "Fira Mono, monospace".to_owned(),
        ..TemplateOptions::default()
    };
    let mut buffer = vec![];
    Template::new(options)
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(styles), "{buffer}");
    assert!(
        buffer.contains("font: 14px Fira Mono, monospace;"),
        "{buffer}"
    );
}
