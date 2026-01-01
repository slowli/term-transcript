//! Test for color diffs.

use super::*;
use crate::{
    style::{Ansi, Style},
    svg::write::{LineWriter, StyledSpan},
};

#[test]
fn getting_spans_basics() {
    let spans = ColorSpan::parse("Apr 18 12:54\n\u{1b}[0m\u{1b}[33m.\u{1b}[0m").unwrap();

    assert_eq!(spans.len(), 2);
    assert!(spans[0].style.is_none());
    assert_eq!(spans[0].len, 13);
    assert_eq!(
        spans[1].style,
        Style {
            fg: Some(Color::YELLOW),
            ..Style::default()
        }
    );
    assert_eq!(spans[1].len, 1);
}

#[test]
fn newlines_break_styling() {
    let spans = ColorSpan::parse("\u{1b}[33mHello\nworld!\u{1b}[0m").unwrap();
    assert_eq!(spans.len(), 3);
    assert_eq!(
        spans[0].style,
        Style {
            fg: Some(Color::YELLOW),
            ..Style::default()
        }
    );
    assert_eq!(spans[0].len, 5);
    assert_eq!(spans[1].style, Style::default());
    assert_eq!(spans[1].len, 1);
    assert_eq!(
        spans[2].style,
        Style {
            fg: Some(Color::YELLOW),
            ..Style::default()
        }
    );
    assert_eq!(spans[2].len, 6);
}

#[test]
fn creating_color_diff_basics() {
    let lhs = [ColorSpan {
        len: 5,
        style: Style::default(),
    }];
    let red = Style {
        fg: Some(Color::RED),
        ..Style::default()
    };
    let rhs = [
        ColorSpan {
            len: 2,
            style: Style::default(),
        },
        ColorSpan { len: 3, style: red },
    ];

    let color_diff = ColorDiff::new(&lhs, &rhs);

    assert_eq!(color_diff.differing_spans.len(), 1);
    let diff_span = &color_diff.differing_spans[0];
    assert_eq!(diff_span.start, 2);
    assert_eq!(diff_span.len, 3);
    assert_eq!(diff_span.lhs_color_spec, Style::default());
    assert_eq!(diff_span.rhs_color_spec, red);
}

#[test]
fn creating_color_diff_overlapping_spans() {
    let red = Style {
        fg: Some(Color::RED),
        ..Style::default()
    };
    let blue = Style {
        bg: Some(Color::BLUE),
        ..Style::default()
    };

    let lhs = [
        ColorSpan {
            len: 2,
            style: Style::default(),
        },
        ColorSpan { len: 3, style: red },
    ];
    let rhs = [
        ColorSpan {
            len: 1,
            style: Style::default(),
        },
        ColorSpan { len: 2, style: red },
        ColorSpan {
            len: 2,
            style: blue,
        },
    ];

    let color_diff = ColorDiff::new(&lhs, &rhs);
    assert_eq!(color_diff.differing_spans.len(), 2);
    assert_eq!(color_diff.differing_spans[0].start, 1);
    assert_eq!(color_diff.differing_spans[0].len, 1);
    assert_eq!(
        color_diff.differing_spans[0].lhs_color_spec,
        Style::default()
    );
    assert_eq!(color_diff.differing_spans[0].rhs_color_spec, red);
    assert_eq!(color_diff.differing_spans[1].start, 3);
    assert_eq!(color_diff.differing_spans[1].len, 2);
    assert_eq!(color_diff.differing_spans[1].lhs_color_spec, red);
    assert_eq!(color_diff.differing_spans[1].rhs_color_spec, blue);
}

fn color_spec_to_string(spec: &Style) -> String {
    let mut buffer = String::new();
    ColorDiff::write_color_spec(&mut buffer, spec).unwrap();
    buffer
}

#[test]
fn writing_color_spec() {
    let mut spec = Style {
        bold: true,
        fg: Some(Color::CYAN),
        ..Style::default()
    };
    let spec_string = color_spec_to_string(&spec);
    assert_eq!(spec_string, "b---     cyan/(none)  ");

    spec.underline = true;
    spec.bg = Some(Color::Index(11));
    let spec_string = color_spec_to_string(&spec);
    assert_eq!(spec_string, "b-u-     cyan/yellow* ");

    spec.italic = true;
    spec.bold = false;
    spec.fg = Some(Color::Rgb(RgbColor(0xc0, 0xff, 0xee)));
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

    let red = Style {
        bold: true,
        fg: Some(Color::RED),
        bg: Some(Color::WHITE),
        ..Style::default()
    };
    let color_diff = ColorDiff {
        differing_spans: vec![DiffColorSpan {
            start: 0,
            len: 2,
            lhs_color_spec: Style::default(),
            rhs_color_spec: red,
        }],
    };

    let mut buffer = vec![];
    let mut out = Ansi::new(&mut buffer, false);
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
        lhs_color_spec: Style::default(),
        rhs_color_spec: Style::default(),
    }
}

#[test]
fn highlighting_diff_on_text() {
    let green = Style {
        fg: Some(Color::GREEN),
        ..Style::default()
    };
    let color_spans = [
        ColorSpan {
            len: 2,
            style: Style::default(),
        },
        ColorSpan {
            len: 11,
            style: green,
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
                    fg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "He".into(),
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(2)),
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
                    fg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(7)),
                    bg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "^^".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(0)),
                    bg: Some(Color::Index(3)),
                    ..Style::default()
                },
                text: "!!".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(7)),
                    bg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "^".into(),
            },
            "     ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(7)),
                    bg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "^".into(),
            },
        ]
    );
}

#[test]
fn spans_on_multiple_lines() {
    let green = Style {
        fg: Some(Color::GREEN),
        ..Style::default()
    };
    let color_spans = [
        ColorSpan {
            len: 9,
            style: green,
        },
        ColorSpan {
            len: 4,
            style: Style::default(),
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
                    fg: Some(Color::Index(2)),
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
                    fg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(2)),
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
                    fg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "  ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(7)),
                    bg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "^^^".into(),
            },
        ]
    );
}

#[test]
fn spans_with_multiple_sequential_line_breaks() {
    let green = Style {
        fg: Some(Color::GREEN),
        ..Style::default()
    };
    let color_spans = [
        ColorSpan {
            len: 6,
            style: green,
        },
        ColorSpan {
            len: 4,
            style: Style::default(),
        },
        ColorSpan {
            len: 4,
            style: green,
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
                    fg: Some(Color::Index(2)),
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
                    fg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "wo".into(),
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(2)),
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
                    fg: Some(Color::Index(1)),
                    ..Style::default()
                },
                text: "> ".into(),
            },
            "  ".into(),
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(7)),
                    bg: Some(Color::Index(1)),
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
        style: Style::default(),
    };
    let mut buffer = vec![];
    color_diff
        .highlight_text(&mut Ansi::new(&mut buffer, false), text, &[color_span])
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
    ColorDiff::highlight_line(&mut Ansi::new(&mut buffer, false), &mut spans, 0, line).unwrap();

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
