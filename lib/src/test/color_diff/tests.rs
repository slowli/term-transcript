//! Test for color diffs.

use termcolor::NoColor;

use super::*;
use crate::svg::write::{LineWriter, Style, StyledSpan};

#[test]
fn getting_spans_basics() {
    let spans = ColorSpan::parse("Apr 18 12:54\n\u{1b}[0m\u{1b}[33m.\u{1b}[0m").unwrap();

    assert_eq!(spans.len(), 2);
    assert!(spans[0].color_spec.is_none());
    assert_eq!(spans[0].len, 13);
    assert_eq!(
        spans[1].color_spec,
        *ColorSpec::new().set_fg(Some(Color::Yellow))
    );
    assert_eq!(spans[1].len, 1);
}

#[test]
fn newlines_break_styling() {
    let spans = ColorSpan::parse("\u{1b}[33mHello\nworld!\u{1b}[0m").unwrap();
    assert_eq!(spans.len(), 3);
    assert_eq!(
        spans[0].color_spec,
        *ColorSpec::new().set_fg(Some(Color::Yellow))
    );
    assert_eq!(spans[0].len, 5);
    assert_eq!(spans[1].color_spec, ColorSpec::default());
    assert_eq!(spans[1].len, 1);
    assert_eq!(
        spans[2].color_spec,
        *ColorSpec::new().set_fg(Some(Color::Yellow))
    );
    assert_eq!(spans[2].len, 6);
}

#[test]
fn creating_color_diff_basics() {
    let lhs = [ColorSpan {
        len: 5,
        color_spec: ColorSpec::default(),
    }];
    let mut red = ColorSpec::new();
    red.set_fg(Some(Color::Red));
    let rhs = [
        ColorSpan {
            len: 2,
            color_spec: ColorSpec::default(),
        },
        ColorSpan {
            len: 3,
            color_spec: red.clone(),
        },
    ];

    let color_diff = ColorDiff::new(&lhs, &rhs);

    assert_eq!(color_diff.differing_spans.len(), 1);
    let diff_span = &color_diff.differing_spans[0];
    assert_eq!(diff_span.start, 2);
    assert_eq!(diff_span.len, 3);
    assert_eq!(diff_span.lhs_color_spec, ColorSpec::default());
    assert_eq!(diff_span.rhs_color_spec, red);
}

#[test]
fn creating_color_diff_overlapping_spans() {
    let mut red = ColorSpec::new();
    red.set_fg(Some(Color::Red));
    let mut blue = ColorSpec::new();
    blue.set_bg(Some(Color::Blue));

    let lhs = [
        ColorSpan {
            len: 2,
            color_spec: ColorSpec::default(),
        },
        ColorSpan {
            len: 3,
            color_spec: red.clone(),
        },
    ];
    let rhs = [
        ColorSpan {
            len: 1,
            color_spec: ColorSpec::default(),
        },
        ColorSpan {
            len: 2,
            color_spec: red.clone(),
        },
        ColorSpan {
            len: 2,
            color_spec: blue.clone(),
        },
    ];

    let color_diff = ColorDiff::new(&lhs, &rhs);
    assert_eq!(color_diff.differing_spans.len(), 2);
    assert_eq!(color_diff.differing_spans[0].start, 1);
    assert_eq!(color_diff.differing_spans[0].len, 1);
    assert_eq!(
        color_diff.differing_spans[0].lhs_color_spec,
        ColorSpec::default()
    );
    assert_eq!(color_diff.differing_spans[0].rhs_color_spec, red);
    assert_eq!(color_diff.differing_spans[1].start, 3);
    assert_eq!(color_diff.differing_spans[1].len, 2);
    assert_eq!(color_diff.differing_spans[1].lhs_color_spec, red);
    assert_eq!(color_diff.differing_spans[1].rhs_color_spec, blue);
}

fn color_spec_to_string(spec: &ColorSpec) -> String {
    let mut buffer = vec![];
    let mut out = NoColor::new(&mut buffer);
    ColorDiff::write_color_spec(&mut out, spec).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[test]
fn writing_color_spec() {
    let mut spec = ColorSpec::new();
    spec.set_bold(true);
    spec.set_fg(Some(Color::Cyan));
    let spec_string = color_spec_to_string(&spec);
    assert_eq!(spec_string, "b---     cyan/(none)  ");

    spec.set_underline(true);
    spec.set_bg(Some(Color::Ansi256(11)));
    let spec_string = color_spec_to_string(&spec);
    assert_eq!(spec_string, "b-u-     cyan/yellow* ");

    spec.set_italic(true);
    spec.set_bold(false);
    spec.set_fg(Some(Color::Rgb(0xc0, 0xff, 0xee)));
    let spec_string = color_spec_to_string(&spec);
    assert_eq!(spec_string, "-iu-  #c0ffee/yellow* ");
}

#[test]
fn writing_color_diff_table() {
    const EXPECTED_TABLE_LINES: &[&str] = &[
        "Positions      Expected style          Actual style     ",
        "========== ====================== ======================",
        "      0..2 ----   (none)/(none)   b---      red/white   ",
    ];

    let mut red = ColorSpec::new();
    red.set_bold(true)
        .set_fg(Some(Color::Red))
        .set_bg(Some(Color::White));
    let color_diff = ColorDiff {
        differing_spans: vec![DiffColorSpan {
            start: 0,
            len: 2,
            lhs_color_spec: ColorSpec::default(),
            rhs_color_spec: red,
        }],
    };

    let mut buffer = vec![];
    let mut out = NoColor::new(&mut buffer);
    color_diff.write_as_table(&mut out).unwrap();
    let table_string = String::from_utf8(buffer).unwrap();

    for (actual, &expected) in table_string.lines().zip(EXPECTED_TABLE_LINES) {
        assert_eq!(actual, expected);
    }
}

fn diff_span(start: usize, len: usize) -> DiffColorSpan {
    DiffColorSpan {
        start,
        len,
        lhs_color_spec: ColorSpec::default(),
        rhs_color_spec: ColorSpec::default(),
    }
}

#[test]
fn highlighting_diff_on_text() {
    let mut green = ColorSpec::default();
    green.set_fg(Some(Color::Green));
    let color_spans = [
        ColorSpan {
            len: 2,
            color_spec: ColorSpec::default(),
        },
        ColorSpan {
            len: 11,
            color_spec: green,
        },
    ];
    let color_diff = ColorDiff {
        differing_spans: vec![
            diff_span(0, 2),
            diff_span(2, 2),
            diff_span(4, 1),
            diff_span(10, 1),
        ],
    };

    let mut out = LineWriter::new(None);
    color_diff
        .highlight_text(&mut out, "Hello, world!", &color_spans)
        .unwrap();
    let lines = out.into_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "He".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(2)),
                    ..Style::default()
                },
                text: "llo, world!".into(),
            },
        ]
    );

    assert_eq!(
        lines[1].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(7)),
                    bg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "^^".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(0)),
                    bg: Some(IndexOrRgb::Index(3)),
                    ..Style::default()
                },
                text: "!!".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(7)),
                    bg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "^".into(),
            },
            "     ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(7)),
                    bg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "^".into(),
            },
        ]
    );
}

#[test]
fn spans_on_multiple_lines() {
    let mut green = ColorSpec::default();
    green.set_fg(Some(Color::Green));
    let color_spans = [
        ColorSpan {
            len: 9,
            color_spec: green,
        },
        ColorSpan {
            len: 4,
            color_spec: ColorSpec::default(),
        },
    ];

    let color_diff = ColorDiff {
        differing_spans: vec![diff_span(9, 3)],
    };

    let mut out = LineWriter::new(None);
    color_diff
        .highlight_text(&mut out, "Hello,\nworld!", &color_spans)
        .unwrap();

    let lines = out.into_lines();
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[0].spans,
        [
            "= ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(2)),
                    ..Style::default()
                },
                text: "Hello,".into(),
            },
        ]
    );

    assert_eq!(
        lines[1].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(2)),
                    ..Style::default()
                },
                text: "wo".into(),
            },
            "rld!".into(),
        ]
    );

    assert_eq!(
        lines[2].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "  ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(7)),
                    bg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "^^^".into(),
            },
        ]
    );
}

#[test]
fn spans_with_multiple_sequential_line_breaks() {
    let mut green = ColorSpec::default();
    green.set_fg(Some(Color::Green));
    let color_spans = [
        ColorSpan {
            len: 6,
            color_spec: green.clone(),
        },
        ColorSpan {
            len: 4,
            color_spec: ColorSpec::default(),
        },
        ColorSpan {
            len: 4,
            color_spec: green,
        },
    ];

    let color_diff = ColorDiff {
        differing_spans: vec![diff_span(10, 3)],
    };

    let mut out = LineWriter::new(None);
    color_diff
        .highlight_text(&mut out, "Hello,\n\nworld!", &color_spans)
        .unwrap();

    let lines = out.into_lines();
    assert_eq!(lines.len(), 4);

    assert_eq!(
        lines[0].spans,
        [
            "= ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(2)),
                    ..Style::default()
                },
                text: "Hello,".into(),
            },
        ]
    );

    assert_eq!(lines[1].spans, ["= ".into()]);

    assert_eq!(
        lines[2].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "wo".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(2)),
                    ..Style::default()
                },
                text: "rld!".into(),
            },
        ]
    );

    assert_eq!(
        lines[3].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "  ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(IndexOrRgb::Index(7)),
                    bg: Some(IndexOrRgb::Index(1)),
                    ..Style::default()
                },
                text: "^^^".into(),
            },
        ]
    );
}

fn test_highlight(color_diff: &ColorDiff, text: &str) -> String {
    let color_span = ColorSpan {
        len: text.len(),
        color_spec: ColorSpec::default(),
    };
    let mut buffer = vec![];
    color_diff
        .highlight_text(&mut NoColor::new(&mut buffer), text, &[color_span])
        .unwrap();
    String::from_utf8(buffer).unwrap()
}

#[test]
fn plaintext_highlight_simple() {
    let color_diff = ColorDiff {
        differing_spans: vec![
            diff_span(0, 2),
            diff_span(2, 2),
            diff_span(4, 1),
            diff_span(10, 1),
        ],
    };

    let buffer = test_highlight(&color_diff, "Hello, world!");
    let expected_buffer = // (prevents formatter from breaking alignment)
        "> Hello, world!\n\
         > ^^!!^     ^\n";
    assert_eq!(buffer, expected_buffer);
}

#[test]
fn plaintext_highlight_with_multiple_lines() {
    let color_diff = ColorDiff {
        differing_spans: vec![diff_span(4, 12)],
    };

    let buffer = test_highlight(&color_diff, "Hello,\nworld!\nMore text");
    let expected_buffer = // (prevents formatter from breaking alignment)
        "> Hello,\n\
         >     ^^\n\
         > world!\n\
         > ^^^^^^\n\
         > More text\n\
         > ^^\n";
    assert_eq!(buffer, expected_buffer);
}

#[test]
fn plaintext_highlight_with_skipped_lines() {
    let color_diff = ColorDiff {
        differing_spans: vec![diff_span(4, 6), diff_span(26, 2)],
    };

    let buffer = test_highlight(&color_diff, "Hello,\nworld!\nMore\ntext\nhere");
    let expected_buffer = // (prevents formatter from breaking alignment)
        "> Hello,\n\
         >     ^^\n\
         > world!\n\
         > ^^^\n\
         = More\n\
         = text\n\
         > here\n\
         >   ^^\n";
    assert_eq!(buffer, expected_buffer);
}

#[test]
fn highlighting_works_with_non_ascii_text() {
    let mut buffer = vec![];
    let line = "  ┌─ Snippet #1:1:1";
    let spans = vec![HighlightedSpan {
        start: 2,
        len: 6,
        kind: SpanHighlightKind::Main,
    }];
    let mut spans = spans.into_iter().peekable();
    ColorDiff::highlight_line(&mut NoColor::new(&mut buffer), &mut spans, 0, line).unwrap();

    let highlight_line = String::from_utf8(buffer).unwrap();
    assert_eq!(highlight_line, "  ^^\n");
}

#[test]
fn plaintext_highlight_with_non_ascii_text() {
    let text = "error[EVAL]: Variable `foo` is not defined\n  \
      ┌─ Snippet #1:1:1\n  \
      │\n\
    1 │ foo(3)\n  \
      │ ^^^ Undefined variable occurrence";

    let color_diff = ColorDiff {
        differing_spans: vec![
            diff_span(45, 6),
            diff_span(69, 3),
            diff_span(73, 1),
            diff_span(75, 3),
            diff_span(88, 3),
        ],
    };

    let buffer = test_highlight(&color_diff, text);
    let expected_buffer = "= error[EVAL]: Variable `foo` is not defined\n\
    >   ┌─ Snippet #1:1:1\n\
    >   ^^\n\
    >   │\n\
    >   ^\n\
    > 1 │ foo(3)\n\
    > ^ ^\n\
    >   │ ^^^ Undefined variable occurrence\n\
    >   ^\n";
    assert_eq!(buffer, expected_buffer);
}
