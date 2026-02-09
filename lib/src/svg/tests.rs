//! Tests for the SVG rendering logic.

use std::{borrow::Cow, convert::Infallible, num::NonZeroUsize};

use anstyle::RgbColor;
use styled_str::{StyledString, styled};
use test_casing::{Product, test_casing};

use super::{
    data::{SerdeStyle, SerdeStyledSpan},
    options::{LineNumberingOptions, WindowOptions},
    *,
};
use crate::{ExitStatus, Interaction, UserInput, svg::data::SerdeColor};

#[test]
fn rendering_simple_transcript() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("test --arg && true"),
        styled!("Hello, [[red on green]]<world>[[/]]!").into(),
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
    assert!(
        buffer.contains(r#"Hello, <span class="fg2">world</span>!"#),
        "{buffer}"
    );
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("test --arg && true"),
        styled!("Hello, [[red on green]]<world>[[/]]!").into(),
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
        styled!("[[dim]]Hello,[[/]] [[blink]]world[[/]]!").into(),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let options = TemplateOptions {
        window: Some(WindowOptions::default()),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let options = TemplateOptions {
        window: Some(WindowOptions::default()),
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
    transcript.add_interaction(UserInput::command("test"), StyledString::default());
    transcript.add_interaction(
        UserInput::command("test --arg"),
        styled!("Hello, [[red on green]]world[[/]]!").into(),
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
    let interaction = Interaction::new("test", styled!("Hello, [[green]]world[[/]]!").into())
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
    let interaction = Interaction::new("test", styled!("Hello, [[green]]world[[/]]!").into())
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
    let interaction = Interaction::new("test", styled!("Hello, [[green]]world[[/]]!").into())
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        window: Some(WindowOptions {
            title: "Window Title".to_owned(),
        }),
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();
    assert!(buffer.contains("<circle"), "{buffer}");

    assert!(buffer.contains(r#"<text x="50%" y="-3.5" "#), "{buffer}");
    assert!(buffer.contains(">Window Title</text>"), "{buffer}");
}

#[test]
fn rendering_pure_svg_transcript_with_frame() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        window: Some(WindowOptions {
            title: "Window Title".to_owned(),
        }),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();
    assert!(buffer.contains("<circle"));

    assert!(buffer.contains(r#"<text x="50%" y="-3.5" "#), "{buffer}");
    assert!(buffer.contains(">Window Title</text>"), "{buffer}");
}

#[test]
fn rendering_transcript_with_animation() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        iter::repeat_n(styled!("Hello, [[green]]world[[/]]!\n"), 22).collect(),
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
        iter::repeat_n(styled!("Hello, [[green]]world[[/]]!\n"), 22).collect(),
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
        iter::repeat_n(styled!("Hello, [[green]]world[[/]]!\n"), 22).collect(),
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
        line_numbers: line_numbers.then(|| LineNumberingOptions {
            scope: LineNumbers::Continuous,
            ..LineNumberingOptions::default()
        }),
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

const TEST_WRAP_OPTIONS: WrapOptions = WrapOptions::HardBreakAt {
    chars: NonZeroUsize::new(5).unwrap(),
    mark: Cow::Borrowed("..."),
};

#[test]
fn rendering_transcript_with_wraps() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        wrap: Some(TEST_WRAP_OPTIONS),
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{buffer}");
    assert!(buffer.contains("class=\"hard-br\""), "{buffer}");
    assert!(buffer.contains(".hard-br:before"), "{buffer}");
    assert!(buffer.contains("content: '...';"), "{buffer}");
}

#[test]
fn rendering_svg_transcript_with_wraps() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        wrap: Some(TEST_WRAP_OPTIONS),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    assert!(buffer.contains(r#"viewBox="0 0 720 102""#), "{buffer}");
    assert!(
        buffer.contains(r#"<g class="container fg7 hard-br"><text x="58" y="41.5">...</text><text x="58" y="59.5">...</text></g>"#),
        "{buffer}"
    );
}

const CONTINUED_NUMBERS: [ContinuedLineNumbers; 3] = [
    ContinuedLineNumbers::Inherit,
    ContinuedLineNumbers::mark(""),
    ContinuedLineNumbers::mark(">>"),
];

#[test_casing(3, CONTINUED_NUMBERS)]
fn rendering_transcript_with_breaks_and_line_numbers(#[map(ref)] continued: &ContinuedLineNumbers) {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        wrap: Some(TEST_WRAP_OPTIONS),
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::EachOutput,
            continued: continued.clone(),
        }),
        ..TemplateOptions::default()
    };
    Template::new(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let expected_numbers = match continued {
        ContinuedLineNumbers::Inherit => r#"<pre class="line-numbers">1<br/>2<br/>3</pre>"#,
        ContinuedLineNumbers::Mark(mark) if mark.is_empty() => {
            r#"<pre class="line-numbers">1<br/><br/></pre>"#
        }
        ContinuedLineNumbers::Mark(_) => {
            r#"<pre class="line-numbers">1<br/><b class="cont"></b><br/><b class="cont"></b></pre>"#
        }
    };
    assert!(buffer.contains(expected_numbers), "{buffer}");
}

#[test_casing(3, CONTINUED_NUMBERS)]
fn rendering_svg_transcript_with_breaks_and_line_numbers(
    #[map(ref)] continued: &ContinuedLineNumbers,
) {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        wrap: Some(TEST_WRAP_OPTIONS),
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::EachOutput,
            continued: continued.clone(),
        }),
        ..TemplateOptions::default()
    };
    Template::pure_svg(options.validated().unwrap())
        .render(&transcript, &mut buffer)
        .unwrap();
    let buffer = String::from_utf8(buffer).unwrap();

    let expected_numbers = match continued {
        ContinuedLineNumbers::Inherit => {
            r#"<g class="container fg7 line-numbers"><text x="32" y="41.5">1</text><text x="32" y="59.5">2</text><text x="32" y="77.5">3</text></g>"#
        }
        ContinuedLineNumbers::Mark(mark) if mark.is_empty() => {
            r#"<g class="container fg7 line-numbers"><text x="32" y="41.5">1</text></g>"#
        }
        ContinuedLineNumbers::Mark(_) => {
            r#"<g class="container fg7 line-numbers"><text x="32" y="41.5">1</text><text x="35" y="59.5">&gt;&gt;</text><text x="35" y="77.5">&gt;&gt;</text></g>"#
        }
    };
    assert!(buffer.contains(expected_numbers), "{buffer}");
}

#[test]
fn rendering_transcript_with_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another_test"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::EachOutput,
            ..LineNumberingOptions::default()
        }),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another_test"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::EachOutput,
            ..LineNumberingOptions::default()
        }),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another_test"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::ContinuousOutputs,
            ..LineNumberingOptions::default()
        }),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::Continuous,
            ..LineNumberingOptions::default()
        }),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("third\ntest").hide(),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::Continuous,
            ..LineNumberingOptions::default()
        }),
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

    let sep_count = buffer
        .lines()
        .filter(|line| line.contains(r#"class="output-sep""#))
        .count();
    assert_eq!(sep_count, 1, "{buffer}");
}

#[test]
fn rendering_transcript_with_input_line_numbers_and_hidden_input_in_pure_svg() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test").hide(),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("third\ntest").hide(),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::Continuous,
            ..LineNumberingOptions::default()
        }),
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

    let sep_count = buffer
        .lines()
        .filter(|line| line.contains(r#"<line class="output-sep""#))
        .count();
    assert_eq!(sep_count, 1, "{buffer}");
}

#[test]
fn rendering_pure_svg_transcript_with_input_line_numbers() {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("Hello, [[green]]world[[/]]!").into(),
    );
    transcript.add_interaction(
        UserInput::command("another\ntest"),
        styled!("Hello,\n[[green]]world[[/]]!").into(),
    );

    let mut buffer = vec![];
    let options = TemplateOptions {
        line_height: Some(18.0 / 14.0),
        line_numbers: Some(LineNumberingOptions {
            scope: LineNumbers::Continuous,
            ..LineNumberingOptions::default()
        }),
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
        styled!("Hello, [[green]]world[[/]]!").into(),
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
struct TestEmbedder {
    with_line_numbers: bool,
}

impl FontEmbedder for TestEmbedder {
    type Error = Infallible;

    fn embed_font(&self, used_chars: BTreeSet<char>) -> Result<EmbeddedFont, Self::Error> {
        let mut expected_chars: BTreeSet<_> = "$ test Hello, world! —".chars().collect();
        if self.with_line_numbers {
            expected_chars.extend("0123456789…".chars());
        }
        assert_eq!(used_chars, expected_chars);

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

#[test_casing(4, Product(([false, true], [false, true])))]
fn embedding_font(pure_svg: bool, with_line_numbers: bool) {
    let mut transcript = Transcript::new();
    transcript.add_interaction(
        UserInput::command("test"),
        styled!("H[[on blue]]ell[[/]]o, [[green]]world[[/]]!").into(),
    );

    let options = TemplateOptions {
        font_family: "./FiraMono.ttf".to_owned(),
        wrap: Some(WrapOptions::HardBreakAt {
            chars: WrapOptions::default_width(),
            mark: "—".into(),
        }),
        line_numbers: with_line_numbers.then(|| LineNumberingOptions {
            continued: ContinuedLineNumbers::mark("…"),
            ..LineNumberingOptions::default()
        }),
        ..TemplateOptions::default().with_font_embedder(TestEmbedder { with_line_numbers })
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

    // Check the background box positioning
    if pure_svg && !with_line_numbers {
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
    let data = serde_json::json!(SerdeStyledSpan {
        style: SerdeStyle::default(),
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, "Test");

    let mut style = SerdeStyle {
        bold: true,
        underline: true,
        fg: Some(SerdeColor::Index(2)),
        bg: Some(SerdeColor::Rgb(RgbColor(0xcc, 0xff, 0xcc))),
        ..SerdeStyle::default()
    };
    let data = serde_json::json!(SerdeStyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(
        rendered,
        "<span class=\"bold underline fg2\" style=\"background: #cfc;\">Test</span>"
    );

    style.bg = None;
    style.underline = false;
    let data = serde_json::json!(SerdeStyledSpan {
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

    let mut data = SerdeStyledSpan {
        style: SerdeStyle {
            inverted: true,
            ..SerdeStyle::default()
        },
        text: "Test",
    };
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, r#"<span class="inv fg-none bg-none">Test</span>"#);

    data.style.fg = Some(SerdeColor::Index(5));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>html_span}}", &data)
        .unwrap();
    assert_eq!(rendered, r#"<span class="inv fg-none bg5">Test</span>"#);

    data.style.bg = Some(SerdeColor::Rgb(RgbColor(0xc0, 0xff, 0xee)));
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

    let data = SerdeStyledSpan {
        style: SerdeStyle {
            blink: true,
            inverted: true,
            ..SerdeStyle::default()
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

    let data = SerdeStyledSpan {
        style: SerdeStyle {
            dimmed: true,
            strikethrough: true,
            ..SerdeStyle::default()
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
    let data = serde_json::json!(SerdeStyledSpan {
        style: SerdeStyle::default(),
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, "");

    let mut style = SerdeStyle {
        bold: true,
        underline: true,
        fg: Some(SerdeColor::Index(2)),
        bg: Some(SerdeColor::Rgb(RgbColor(0xcc, 0xff, 0xcc))),
        ..SerdeStyle::default()
    };
    let data = serde_json::json!(SerdeStyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, " class=\"bold underline fg2 bg#cfc\"");

    style.bg = Some(SerdeColor::Index(0));
    style.underline = false;
    style.dimmed = true;
    let data = serde_json::json!(SerdeStyledSpan {
        style,
        text: "Test",
    });
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, " class=\"bold dimmed fg2 bg0\"");

    style.fg = Some(SerdeColor::Rgb(RgbColor(0xc0, 0xff, 0xee)));
    style.bg = None;
    style.dimmed = false;
    let data = serde_json::json!(SerdeStyledSpan {
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

    let mut data = SerdeStyledSpan {
        style: SerdeStyle {
            inverted: true,
            ..SerdeStyle::default()
        },
        text: "Test",
    };
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, r#" class="inv fg-none bg-none""#);

    data.style.fg = Some(SerdeColor::Index(5));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, r#" class="inv fg-none bg5""#);

    data.style.bg = Some(SerdeColor::Rgb(RgbColor(0xc0, 0xff, 0xee)));
    let rendered = handlebars
        .render_template("{{>_helpers}}\n{{>svg_tspan_attrs}}", &data)
        .unwrap();
    assert_eq!(rendered, r#" class="inv bg5" style="fill: #c0ffee;""#);
}
