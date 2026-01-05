//! Tests for the SVG rendering logic.

use std::convert::Infallible;

use test_casing::test_casing;

use super::*;
use crate::{
    style::{Color, Style, StyledSpan},
    ExitStatus, Interaction, UserInput,
};

#[test]
fn parsing_scroll_options() {
    let json = serde_json::json!({});
    let options: ScrollOptions = serde_json::from_value(json).unwrap();
    assert_eq!(options, ScrollOptions::DEFAULT);

    let json = serde_json::json!({
        "pixels_per_scroll": 40,
        "elision_threshold": 0.1,
    });
    let options: ScrollOptions = serde_json::from_value(json).unwrap();
    assert_eq!(
        options,
        ScrollOptions {
            pixels_per_scroll: NonZeroUsize::new(40).unwrap(),
            elision_threshold: 0.1,
            ..ScrollOptions::DEFAULT
        }
    );
}

#[test]
fn validating_options() {
    // Default options must be valid.
    TemplateOptions::default().validate().unwrap();

    let bogus_options = TemplateOptions {
        line_height: Some(-1.0),
        ..TemplateOptions::default()
    };
    let err = bogus_options.validate().unwrap_err().to_string();
    assert!(err.contains("line_height"), "{err}");

    let bogus_options = TemplateOptions {
        advance_width: Some(-1.0),
        ..TemplateOptions::default()
    };
    let err = bogus_options.validate().unwrap_err().to_string();
    assert!(err.contains("advance_width"), "{err}");

    let bogus_options = TemplateOptions {
        scroll: Some(ScrollOptions {
            interval: -1.0,
            ..ScrollOptions::default()
        }),
        ..TemplateOptions::default()
    };
    let err = format!("{:#}", bogus_options.validate().unwrap_err());
    assert!(err.contains("interval"), "{err}");

    for elision_threshold in [-1.0, 1.0] {
        let bogus_options = TemplateOptions {
            scroll: Some(ScrollOptions {
                elision_threshold,
                ..ScrollOptions::default()
            }),
            ..TemplateOptions::default()
        };
        let err = format!("{:#}", bogus_options.validate().unwrap_err());
        assert!(err.contains("elision_threshold"), "{err}");
    }
}

#[test]
fn rendering_simple_transcript() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("test --arg && true"),
        "Hello, \u{1b}[31m\u{1b}[42m<world>\u{1b}[0m!",
    );

    let mut buffer = vec![];
    Template::default()
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

    let second_input_span = "test --arg &amp;&amp; true";
    assert!(buffer.contains(second_input_span), "{buffer}");
    let second_output_span = r#"<span class="fg1 bg2">&lt;world&gt;</span>"#;
    assert!(buffer.contains(second_output_span), "{buffer}");
}

#[test]
fn rendering_simple_transcript_to_pure_svg() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("test --arg && true"),
        "Hello, \u{1b}[31m\u{1b}[42m<world>\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let top_svg = "<svg viewBox=\"0 0 720 118\"";
    assert!(buffer.contains(top_svg), "{buffer}");
    let first_input_text = r#"<g class="input"><g transform="translate(10 15.5)">"#;
    assert!(buffer.contains(first_input_text), "{buffer}");
    let second_input_span = r#"<text x="8" textLength="152"> test --arg &amp;&amp; true</text"#;
    assert!(buffer.contains(second_input_span), "{buffer}");

    let first_output_text =
        r#"<g class="output"><g transform="translate(10 41.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(first_output_text), "{buffer}");
    let second_input_text = r#"<g class="input"><g transform="translate(10 67.5)">"#;
    assert!(buffer.contains(second_input_text), "{buffer}");
    let second_output_text =
        r#"<g class="output"><g transform="translate(10 93.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(second_output_text), "{buffer}");
    let second_output_span = r#"<text x="56" textLength="56" class="fg1 bg2">&lt;world&gt;</text>"#;
    assert!(buffer.contains(second_output_span), "{buffer}");
}

#[test]
fn rendering_transcript_with_opacity_options() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "\u{1b}[2mHello,\u{1b}[0m \u{1b}[5mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        dim_opacity: 0.5,
        blink: BlinkOptions {
            interval: 0.4,
            opacity: 0.0,
        },
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(
        buffer.contains(
            ".dimmed > span { color: color-mix(in hsl, currentColor 50%, transparent); }"
        ),
        "{buffer}"
    );
    // Parts of the `@keyframes blink`
    assert!(
        buffer.contains("color: color-mix(in hsl, currentColor 0%, transparent);"),
        "{buffer}"
    );
    assert!(
        buffer.contains("background: rgb(from #1c1c1c r g b / 100%);"),
        "{buffer}"
    );
    assert!(
        buffer.contains(".blink > span { animation: 0.8s steps(2, jump-none) 0s infinite blink; }"),
        "{buffer}"
    );
}

#[test]
fn rendering_transcript_with_hidden_input() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test").hide(),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let options = TemplateOptions {
        window_frame: true,
        line_height: Some(18.0 / 14.0),
        ..TemplateOptions::default()
    };
    let mut buffer = vec![];
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 -22 720 60""#), "{buffer}");
    assert!(buffer.contains(r#"viewBox="0 0 720 18""#), "{buffer}");
    assert!(
        buffer.contains(r#"<div class="input input-hidden">"#),
        "{buffer}"
    );
}

#[test]
fn rendering_transcript_with_hidden_input_to_pure_svg() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test").hide(),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let options = TemplateOptions {
        window_frame: true,
        line_height: Some(18.0 / 14.0),
        advance_width: Some(0.575), // slightly decreased value
        ..TemplateOptions::default()
    };
    let mut buffer = vec![];
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 -22 720 60""#), "{buffer}");
    assert!(buffer.contains(r#"viewBox="0 0 720 18""#), "{buffer}");
    // No background for input should be displayed.
    assert!(buffer.contains(r#"<g class="input-bg"></g>"#), "{buffer}");
    let output_span =
        r#"<g class="output"><g transform="translate(10 13.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(output_span), "{buffer}");
    assert!(!buffer.contains(r#"class="input""#), "{buffer}");
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
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let top_svg = "<svg viewBox=\"0 0 720 94\"";
    assert!(buffer.contains(top_svg), "{buffer}");
    let second_input_bg = r#"<rect x="0" y="28" width="100%" height="22""#;
    assert!(buffer.contains(second_input_bg), "{buffer}");
    let second_input_text = r#"<g class="input"><g transform="translate(10 43.5)">"#;
    assert!(buffer.contains(second_input_text), "{buffer}");
    let second_output_text =
        r#"<g class="output"><g transform="translate(10 69.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(second_output_text), "{buffer}");
}

#[test]
fn rendering_transcript_with_explicit_success() {
    let mut transcript = Transcript::new();
    let interaction = Interaction::new("test", "Hello, \u{1b}[32mworld\u{1b}[0m!")
        .with_exit_status(ExitStatus(0));
    transcript.add_existing_interaction(interaction);

    let mut buffer = vec![];
    Template::default()
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
    Template::default()
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
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
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
    Template::new(options.validated().unwrap())
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
    Template::pure_svg(options.validated().unwrap())
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
        line_height: Some(18.0 / 14.0),
        scroll: Some(ScrollOptions {
            max_height: NonZeroUsize::new(240).unwrap(),
            pixels_per_scroll: NonZeroUsize::new(52).unwrap(),
            interval: 3.0,
            ..ScrollOptions::default()
        }),
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 260""#), "{buffer}");
    let scrollbar =
        r#"<rect id="scrollbar" class="scrollbar" x="708" y="10" width="5" height="136" />"#;
    assert!(buffer.contains(scrollbar), "{buffer}");
    let animate_tag = r##"<animate id="scroll" href="#scroll-container" attributeName="viewBox""##;
    assert!(buffer.contains(animate_tag), "{buffer}");
    let expected_view_boxes = "0 0 720 240;0 52 720 240;0 104 720 240;0 156 720 240;0 184 720 240";
    assert!(buffer.contains(expected_view_boxes), "{buffer}");
}

#[test]
fn scrollbar_animation_elision() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!\n".repeat(22),
    );

    for elision_threshold in [0.0, 0.25] {
        println!("Testing elision_threshold={elision_threshold}");

        let mut buffer = vec![];
        let options = TemplateOptions {
            line_height: Some(18.0 / 14.0),
            scroll: Some(ScrollOptions {
                max_height: NonZeroUsize::new(240).unwrap(),
                pixels_per_scroll: NonZeroUsize::new(58).unwrap(),
                interval: 3.0,
                elision_threshold,
                ..ScrollOptions::default()
            }),
            ..TemplateOptions::default()
        };
        Template::new(options.validated().unwrap())
            .render(&transcript, &mut buffer)
            .unwrap();
        let buffer = String::from_utf8(buffer).unwrap();

        let expected_duration = if elision_threshold > 0.0 {
            r#"dur="9s""#
        } else {
            r#"dur="12s""#
        };
        assert!(buffer.contains(expected_duration), "{buffer}");

        let expected_view_boxes = if elision_threshold > 0.0 {
            "0 0 720 240;0 58 720 240;0 116 720 240;0 184 720 240"
        } else {
            "0 0 720 240;0 58 720 240;0 116 720 240;0 174 720 240;0 184 720 240"
        };
        assert!(buffer.contains(expected_view_boxes), "{buffer}");

        let expected_scrollbar_ys = if elision_threshold > 0.0 {
            r#"values="10;42.8;75.6;114""#
        } else {
            r#"values="10;42.8;75.6;108.3;114""#
        };
        assert!(buffer.contains(expected_scrollbar_ys), "{buffer}");
    }
}

#[test_casing(2, [false, true])]
fn rendering_pure_svg_transcript_with_animation(line_numbers: bool) {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "Hello, \u{1b}[32mworld\u{1b}[0m!\n".repeat(22),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        scroll: Some(ScrollOptions {
            max_height: NonZeroUsize::new(240).unwrap(),
            pixels_per_scroll: NonZeroUsize::new(52).unwrap(),
            interval: 3.0,
            ..ScrollOptions::default()
        }),
        line_numbers: line_numbers.then_some(LineNumbers::Continuous),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let view_box = if line_numbers {
        r#"viewBox="0 0 749 260""#
    } else {
        r#"viewBox="0 0 720 260""#
    };
    assert!(buffer.contains(view_box), "{buffer}");
    let scrollbar = if line_numbers {
        r#"<rect id="scrollbar" class="scrollbar" x="737" y="10" width="5" height="136" />"#
    } else {
        r#"<rect id="scrollbar" class="scrollbar" x="708" y="10" width="5" height="136" />"#
    };
    assert!(buffer.contains(scrollbar), "{buffer}");

    let animate_tag = r##"<animate id="scroll" href="#scroll-container" attributeName="viewBox""##;
    assert!(buffer.contains(animate_tag), "{buffer}");
    let expected_view_boxes = if line_numbers {
        "0 0 749 240;0 52 749 240;0 104 749 240;0 156 749 240;0 184 749 240"
    } else {
        "0 0 720 240;0 52 720 240;0 104 720 240;0 156 720 240;0 184 720 240"
    };
    assert!(buffer.contains(expected_view_boxes), "{buffer}");
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
        line_height: Some(18.0 / 14.0),
        wrap: Some(WrapOptions::HardBreakAt(NonZeroUsize::new(5).unwrap())),
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{buffer}");
    assert!(buffer.contains("class=\"hard-br\""), "{buffer}");
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
        line_height: Some(18.0 / 14.0),
        wrap: Some(WrapOptions::HardBreakAt(NonZeroUsize::new(5).unwrap())),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{buffer}");
    assert!(
        buffer.contains(r#"<g class="container fg7 hard-br"><text x="58" y="41.5">»</text><text x="58" y="59.5">»</text></g>"#),
        "{buffer}"
    );
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
    Template::new(options.validated().unwrap())
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
        line_height: Some(18.0 / 14.0),
        line_numbers: Some(LineNumbers::EachOutput),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(".line-numbers {"), "{buffer}");
    let first_output_ln = r#"<text x="32" y="41.5">1</text>"#;
    assert!(buffer.contains(first_output_ln), "{buffer}");
    let second_output_ln1 = r#"<text x="32" y="93.5">1</text>"#;
    assert!(buffer.contains(second_output_ln1), "{buffer}");
    let second_output_ln2 = r#"<text x="32" y="111.5">2</text>"#;
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
    Template::new(options.validated().unwrap())
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
    Template::new(options.validated().unwrap())
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
fn rendering_transcript_with_input_line_numbers_and_hidden_input() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test").hide(),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("third\ntest").hide(),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumbers::Continuous),
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let first_output_line_numbers = r#"<div class="output"><pre class="line-numbers">1</pre>"#;
    assert!(buffer.contains(first_output_line_numbers), "{buffer}");
    let input_line_numbers = r#"<div class="input"><pre class="line-numbers">2<br/>3</pre>"#;
    assert!(buffer.contains(input_line_numbers), "{buffer}");
    let second_output_line_numbers =
        r#"<div class="output"><pre class="line-numbers">4<br/>5</pre>"#;
    assert!(buffer.contains(second_output_line_numbers), "{buffer}");
}

#[test]
fn rendering_transcript_with_input_line_numbers_and_hidden_input_in_pure_svg() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test").hide(),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        "Hello,\n\u{1b}[32mworld\u{1b}[0m!",
    );
    transcript.add_interaction(
        UserInput::command("third\ntest").hide(),
        "Hello, \u{1b}[32mworld\u{1b}[0m!",
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        line_numbers: Some(LineNumbers::Continuous),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let input_bg = r#"<g class="input-bg"><rect x="0" y="24" width="100%" height="40"></rect></g>"#;
    assert!(buffer.contains(input_bg), "{buffer}");
    let line_numbers = "<text x=\"32\" y=\"13.5\">1</text>\
        <text x=\"32\" y=\"39.5\">2</text>\
        <text x=\"32\" y=\"57.5\">3</text>\
        <text x=\"32\" y=\"83.5\">4</text>\
        <text x=\"32\" y=\"101.5\">5</text>\
        <text x=\"32\" y=\"125.5\">6</text>";
    assert!(buffer.contains(line_numbers), "{buffer}");

    let first_output =
        r#"<g class="output"><g transform="translate(39 13.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(first_output), "{buffer}");
    let second_output = r#"<g transform="translate(39 101.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(second_output), "{buffer}");
    let third_output =
        r#"<g class="output"><g transform="translate(39 125.5)" clip-path="xywh(0 0 100% 18px)">"#;
    assert!(buffer.contains(third_output), "{buffer}");
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
        line_height: Some(18.0 / 14.0),
        line_numbers: Some(LineNumbers::Continuous),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let line_numbers = "<text x=\"32\" y=\"15.5\">1</text>\
            <text x=\"32\" y=\"41.5\">2</text>\
            <text x=\"32\" y=\"67.5\">3</text>\
            <text x=\"32\" y=\"85.5\">4</text>\
            <text x=\"32\" y=\"111.5\">5</text>\
            <text x=\"32\" y=\"129.5\">6</text>";
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
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(styles), "{buffer}");
    assert!(
        buffer.contains("font: 14px Fira Mono, monospace;"),
        "{buffer}"
    );
}

#[derive(Debug)]
struct TestEmbedder;

impl FontEmbedder for TestEmbedder {
    type Error = Infallible;

    fn embed_font(&self, used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error> {
        assert_eq!(
            used_chars,
            "$ test Hello, world! »".chars().collect::<BTreeSet<_>>()
        );
        Ok(EmbeddedFont {
            family_name: "Fira Mono".to_owned(),
            metrics: FontMetrics {
                units_per_em: 1_000,
                advance_width: 600,
                ascent: 1_050,
                descent: -350,
                bold_spacing: 0.01,
                italic_spacing: 0.0,
            },
            faces: vec![
                EmbeddedFontFace {
                    is_bold: Some(false),
                    ..EmbeddedFontFace::woff2(b"fira mono".to_vec())
                },
                EmbeddedFontFace {
                    is_bold: Some(true),
                    ..EmbeddedFontFace::woff2(b"fira mono bold".to_vec())
                },
            ],
        })
    }
}

#[test_casing(2, [false, true])]
fn embedding_font(pure_svg: bool) {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        "H\u{1b}[44mell\u{1b}[0mo, \u{1b}[32mworld\u{1b}[0m!",
    );

    let options = TemplateOptions {
        font_family: "./FiraMono.ttf".to_owned(),
        ..TemplateOptions::default().with_font_embedder(TestEmbedder)
    };
    let mut buffer = vec![];
    let template = if pure_svg {
        Template::pure_svg(options.validated().unwrap())
    } else {
        Template::new(options.validated().unwrap())
    };
    template.render(&transcript, &mut buffer).unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains("@font-face"), "{buffer}");
    assert!(buffer.contains("font-family: \"Fira Mono\""), "{buffer}");
    assert!(
        buffer.contains("src: url(\"data:font/woff2;base64,ZmlyYSBtb25v\")"),
        "{buffer}"
    );
    // Bold font face
    assert!(
        buffer.contains("src: url(\"data:font/woff2;base64,ZmlyYSBtb25vIGJvbGQ=\");"),
        "{buffer}"
    );
    assert!(
        buffer.contains("font: 14px \"Fira Mono\", monospace"),
        "{buffer}"
    );
    // Letter spacing adjustment for the bold font face
    assert!(
        buffer.contains(".bold,.prompt { font-weight: bold; letter-spacing: 0.01em; }"),
        "{buffer}"
    );

    if pure_svg {
        assert!(
            buffer.contains(r#"<rect x="18.4" y="29.6" width="25.2" height="19.6" class="fg4"/>"#),
            "{buffer}"
        );
    }
}

#[test]
fn rendering_html_span() {
    let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.register_template("_helpers", helpers);
    let data = serde_json::json!(StyledSpan {
        style: Style::default(),
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, "Test");

    let mut style = Style {
        bold: true,
        underline: true,
        fg: Some(Color::Index(2)),
        bg: Some(Color::Rgb("#cfc".parse().unwrap())),
        ..Style::default()
    };
    let data = serde_json::json!(StyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(
        rendered,
        "<span class=\"bold underline fg2\" style=\"background: #ccffcc;\">Test</span>"
    );

    style.bg = None;
    style.underline = false;
    let data = serde_json::json!(StyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, "<span class=\"bold fg2\">Test</span>");
}

#[test]
fn rendering_inverted_html_span() {
    let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.register_template("_helpers", helpers);

    let mut data = StyledSpan {
        style: Style {
            inverted: true,
            ..Style::default()
        },
        text: "Test",
    };
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, r#"<span class="inv fg-none bg-none">Test</span>"#);

    data.style.fg = Some(Color::Index(5));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, r#"<span class="inv fg-none bg5">Test</span>"#);

    data.style.bg = Some(Color::Rgb("#c0ffee".parse().unwrap()));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(
        rendered,
        r#"<span class="inv bg5" style="color: #c0ffee;">Test</span>"#
    );
}

#[test]
fn rendering_blinking_html_span() {
    let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.register_template("_helpers", helpers);

    let data = StyledSpan {
        style: Style {
            blink: true,
            inverted: true,
            ..Style::default()
        },
        text: "Test",
    };
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(
        rendered,
        r#"<span class="blink inv fg-none bg-none"><span>Test</span></span>"#
    );
}

#[test]
fn rendering_dimmed_html_span() {
    let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.register_template("_helpers", helpers);

    let data = StyledSpan {
        style: Style {
            dimmed: true,
            strikethrough: true,
            ..Style::default()
        },
        text: "Test",
    };
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(
        rendered,
        r#"<span class="strike dimmed"><span>Test</span></span>"#
    );
}

#[test]
fn rendering_svg_tspan() {
    let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.register_template("_helpers", helpers);
    let data = serde_json::json!(StyledSpan {
        style: Style::default(),
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, "");

    let mut style = Style {
        bold: true,
        underline: true,
        fg: Some(Color::Index(2)),
        bg: Some(Color::Rgb("#cfc".parse().unwrap())),
        ..Style::default()
    };
    let data = serde_json::json!(StyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, " class=\"bold underline fg2 bg#ccffcc\"");

    style.bg = Some(Color::Index(0));
    style.underline = false;
    style.dimmed = true;
    let data = serde_json::json!(StyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, " class=\"bold dimmed fg2 bg0\"");

    style.fg = Some(Color::Rgb("#c0ffee".parse().unwrap()));
    style.bg = None;
    style.dimmed = false;
    let data = serde_json::json!(StyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, " class=\"bold\" style=\"fill: #c0ffee;\"");
}

#[test]
fn rendering_inverted_svg_tspan() {
    let helpers = HandlebarsTemplate::compile(COMMON_HELPERS).unwrap();
    let mut handlebars = Handlebars::new();
    register_helpers(&mut handlebars);
    handlebars.register_template("_helpers", helpers);

    let mut data = StyledSpan {
        style: Style {
            inverted: true,
            ..Style::default()
        },
        text: "Test",
    };
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, r#" class="inv fg-none bg-none""#);

    data.style.fg = Some(Color::Index(5));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, r#" class="inv fg-none bg5""#);

    data.style.bg = Some(Color::Rgb("#c0ffee".parse().unwrap()));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, r#" class="inv bg5" style="fill: #c0ffee;""#);
}
