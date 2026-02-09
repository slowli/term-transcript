//! Test for color diffs.

use anstyle::RgbColor;

use super::*;
use crate::{StyledString, styled};

#[test]
fn creating_color_diff_basics() {
    let lhs = [StyledSpan::new(Style::new(), 5)];
    let red = Style::new().fg_color(Some(AnsiColor::Red.into()));
    let rhs = [StyledSpan::new(Style::new(), 2), StyledSpan::new(red, 3)];

    let color_diff = StyleDiff::new(
        StyledStr {
            text: "Hello",
            spans: &lhs,
        },
        StyledStr {
            text: "Hello",
            spans: &rhs,
        },
    );

    assert_eq!(color_diff.differing_spans.len(), 1);
    let diff_span = &color_diff.differing_spans[0];
    assert_eq!(diff_span.start, 2);
    assert_eq!(diff_span.len.get(), 3);
    assert_eq!(diff_span.lhs_style, Style::default());
    assert_eq!(diff_span.rhs_style, red);
}

#[test]
fn creating_color_diff_overlapping_spans() {
    let red = Style::new().fg_color(Some(AnsiColor::Red.into()));
    let blue = Style::new().fg_color(Some(AnsiColor::Blue.into()));

    let lhs = [StyledSpan::new(Style::new(), 2), StyledSpan::new(red, 3)];
    let rhs = [
        StyledSpan::new(Style::new(), 1),
        StyledSpan::new(red, 2),
        StyledSpan::new(blue, 2),
    ];

    let color_diff = StyleDiff::new(
        StyledStr {
            text: "Hello",
            spans: &lhs,
        },
        StyledStr {
            text: "Hello",
            spans: &rhs,
        },
    );
    assert_eq!(color_diff.differing_spans.len(), 2);
    assert_eq!(color_diff.differing_spans[0].start, 1);
    assert_eq!(color_diff.differing_spans[0].len.get(), 1);
    assert_eq!(color_diff.differing_spans[0].lhs_style, Style::default());
    assert_eq!(color_diff.differing_spans[0].rhs_style, red);
    assert_eq!(color_diff.differing_spans[1].start, 3);
    assert_eq!(color_diff.differing_spans[1].len.get(), 2);
    assert_eq!(color_diff.differing_spans[1].lhs_style, red);
    assert_eq!(color_diff.differing_spans[1].rhs_style, blue);
}

#[test]
fn writing_color_spec() {
    let mut spec = Style::new().bold().fg_color(Some(AnsiColor::Cyan.into()));
    let spec_strings = StyleDiff::write_style(&spec);
    assert_eq!(spec_strings, ["bold cyan"]);

    spec = spec
        .underline()
        .bg_color(Some(AnsiColor::BrightYellow.into()));
    let spec_string = StyleDiff::write_style(&spec);
    assert_eq!(spec_string, ["bold underline cyan on", "yellow!"]);

    spec = spec
        .italic()
        .fg_color(Some(RgbColor(0xc0, 0xff, 0xee).into()));
    let spec_string = StyleDiff::write_style(&spec);
    assert_eq!(spec_string, ["bold italic underline", "#c0ffee on yellow!"]);
}

#[test]
fn writing_color_diff_table() {
    const EXPECTED: StyledStr = styled!(
        r"[[bold]]Positions         Left style                Right style       [[*]]
========== ========================= =========================[[/]]
      0..2          (none)           [[bold strike blink red on white]]bold strike blink red on [[/]]
                                     [[bold strike blink red on white]]          white          [[/]]
"
    );

    let red = Style::new()
        .bold()
        .strikethrough()
        .blink()
        .fg_color(Some(AnsiColor::Red.into()))
        .bg_color(Some(AnsiColor::White.into()));
    let color_diff = StyleDiff {
        text: "",       // not used
        lhs_spans: &[], // not used
        differing_spans: vec![DiffStyleSpan {
            start: 0,
            len: NonZeroUsize::new(2).unwrap(),
            lhs_style: Style::default(),
            rhs_style: red,
        }],
    };
    let out = StyledString::from_ansi(&format!("{color_diff:#}")).unwrap();
    assert_eq!(out, EXPECTED);
}

fn diff_span(start: usize, len: usize) -> DiffStyleSpan {
    DiffStyleSpan {
        start,
        len: NonZeroUsize::new(len).unwrap(),
        lhs_style: Style::default(),
        rhs_style: Style::default(),
    }
}

#[test]
fn highlighting_diff_on_text() {
    const EXPECTED: StyledStr = styled!(
        "[[red]]> [[/]]He[[green]]llo, world![[/]]\n\
         [[red]]> [[white on red]]^^[[black on yellow]]!![[white on red]]^[[/]]     [[white on red]]^[[/]]\n"
    );

    let green = Style::new().fg_color(Some(AnsiColor::Green.into()));
    let style_spans = [StyledSpan::new(Style::new(), 2), StyledSpan::new(green, 11)];
    let color_diff = StyleDiff {
        text: "Hello, world!",
        lhs_spans: &style_spans,
        differing_spans: vec![
            diff_span(0, 2),
            diff_span(2, 2),
            diff_span(4, 1),
            diff_span(10, 1),
        ],
    };

    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED);
}

#[test]
fn spans_on_multiple_lines() {
    const EXPECTED: StyledStr = styled!(
        "= [[green]]Hello,[[/]]\n\
         [[red]]> [[green]]wo[[/]]rld!\n\
         [[red]]> [[/]]  [[white on red]]^^^[[/]]\n"
    );

    let green = Style::new().fg_color(Some(AnsiColor::Green.into()));
    let color_spans = [StyledSpan::new(green, 9), StyledSpan::new(Style::new(), 4)];

    let color_diff = StyleDiff {
        text: "Hello,\nworld!",
        lhs_spans: &color_spans,
        differing_spans: vec![diff_span(9, 3)],
    };
    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED);
}

#[test]
fn spans_with_multiple_sequential_line_breaks() {
    const EXPECTED: StyledStr = styled!(
        "= [[green]]Hello,[[/]]\n\
         = \n\
         [[red]]> [[/]]wo[[green]]rld![[/]]\n\
         [[red]]> [[/]]  [[white on red]]^^^[[/]]\n"
    );

    let green = Style::new().fg_color(Some(AnsiColor::Green.into()));
    let color_spans = [
        StyledSpan::new(green, 6),
        StyledSpan::new(Style::new(), 4),
        StyledSpan::new(green, 4),
    ];

    let color_diff = StyleDiff {
        text: "Hello,\n\nworld!",
        lhs_spans: &color_spans,
        differing_spans: vec![diff_span(10, 3)],
    };
    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED);
}

#[test]
fn plaintext_highlight_simple() {
    const EXPECTED: StyledStr = styled!(
        "[[red]]> [[/]]Hello, world!\n\
         [[red]]> [[white on red]]^^[[black on yellow]]!![[white on red]]^[[/]]     [[white on red]]^[[/]]\n"
    );

    let text = "Hello, world!";
    let color_diff = StyleDiff {
        text,
        lhs_spans: &[StyledSpan::new(Style::new(), text.len())],
        differing_spans: vec![
            diff_span(0, 2),
            diff_span(2, 2),
            diff_span(4, 1),
            diff_span(10, 1),
        ],
    };

    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED);
}

#[test]
fn plaintext_highlight_with_multiple_lines() {
    let text = "Hello,\nworld!\nMore text";
    let color_diff = StyleDiff {
        text,
        lhs_spans: &[StyledSpan::new(Style::new(), text.len())],
        differing_spans: vec![diff_span(4, 12)],
    };

    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    let expected_buffer = // (prevents formatter from breaking alignment)
        "> Hello,\n\
         >     ^^\n\
         > world!\n\
         > ^^^^^^\n\
         > More text\n\
         > ^^\n";
    assert_eq!(output.text, expected_buffer);
}

#[test]
fn plaintext_highlight_with_skipped_lines() {
    let text = "Hello,\nworld!\nMore\ntext\nhere";
    let color_diff = StyleDiff {
        text,
        lhs_spans: &[StyledSpan::new(Style::new(), text.len())],
        differing_spans: vec![diff_span(4, 6), diff_span(26, 2)],
    };

    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    let expected_buffer = // (prevents formatter from breaking alignment)
        "> Hello,\n\
         >     ^^\n\
         > world!\n\
         > ^^^\n\
         = More\n\
         = text\n\
         > here\n\
         >   ^^\n";
    assert_eq!(output.text, expected_buffer);
}

#[test]
fn highlighting_works_with_non_ascii_text() {
    // Since we cannot create a `Formatter`, we are forced to use this crutch.
    struct Test;

    impl fmt::Display for Test {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let line = "  ┌─ Snippet #1:1:1";
            let spans = vec![HighlightedSpan {
                start: 2,
                len: NonZeroUsize::new(6).unwrap(),
                kind: SpanHighlightKind::Main,
            }];
            let mut spans = spans.into_iter().peekable();

            StyleDiff::highlight_line(formatter, &mut spans, 0, line)
        }
    }

    let output = StyledString::from_ansi(&Test.to_string()).unwrap();
    assert_eq!(output, styled!("  [[white on red]]^^[[/]]\n"));
}

#[test]
fn plaintext_highlight_with_non_ascii_text() {
    let text = "error[EVAL]: Variable `foo` is not defined\n  \
      ┌─ Snippet #1:1:1\n  \
      │\n\
    1 │ foo(3)\n  \
      │ ^^^ Undefined variable occurrence";

    let color_diff = StyleDiff {
        text,
        lhs_spans: &[StyledSpan::new(Style::new(), text.len())],
        differing_spans: vec![
            diff_span(45, 6),
            diff_span(69, 3),
            diff_span(73, 1),
            diff_span(75, 3),
            diff_span(88, 3),
        ],
    };

    let output = StyledString::from_ansi(&color_diff.to_string()).unwrap();
    let expected_buffer = "= error[EVAL]: Variable `foo` is not defined\n\
    >   ┌─ Snippet #1:1:1\n\
    >   ^^\n\
    >   │\n\
    >   ^\n\
    > 1 │ foo(3)\n\
    > ^ ^\n\
    >   │ ^^^ Undefined variable occurrence\n\
    >   ^\n";
    assert_eq!(output.text, expected_buffer);
}

#[test]
fn whitespace_diff_is_ignored() {
    let diff = StyleDiff::new(
        styled!("[[red]]Hello, [[/]]world"),
        styled!("[[red]]Hello,[[/]] world"),
    );
    assert!(diff.is_empty(), "{diff:#}");

    let diff = StyleDiff::new(
        styled!("[[red]]Hello,\n[[/]]world"),
        styled!("[[red]]Hello,[[/]]\nworld"),
    );
    assert!(diff.is_empty(), "{diff:#}");

    let diff = StyleDiff::new(styled!("Hell[[red]]o, [[/]]world"), styled!("Hello, world"));
    assert_eq!(diff.differing_spans.len(), 1);
    assert_eq!(diff.differing_spans[0].len.get(), 2);
    assert_eq!(diff.differing_spans[0].start, 4);

    let diff = StyleDiff::new(styled!("Hello,[[red]] world"), styled!("Hello, world"));
    assert_eq!(diff.differing_spans.len(), 1);
    assert_eq!(diff.differing_spans[0].len.get(), 5);
    assert_eq!(diff.differing_spans[0].start, 7);
}

#[test]
fn whitespace_is_not_ignored_for_specific_styles() {
    let diff = StyleDiff::new(
        styled!("[[on red]]Hello, [[/]]world"),
        styled!("[[on red]]Hello,[[/]] world"),
    );
    assert_eq!(diff.differing_spans.len(), 1);
    assert_eq!(diff.differing_spans[0].len.get(), 1);
    assert_eq!(diff.differing_spans[0].start, 6);

    let diff = StyleDiff::new(
        styled!("[[on red]]Hello,\n[[/]]world"),
        styled!("[[on red]]Hello,[[/]]\nworld"),
    );
    assert!(diff.is_empty());

    let diff = StyleDiff::new(styled!("Hello, [[ul]]world"), styled!("Hello,[[ul]] world"));
    assert_eq!(diff.differing_spans.len(), 1);
    assert_eq!(diff.differing_spans[0].len.get(), 1);
    assert_eq!(diff.differing_spans[0].start, 6);
}
