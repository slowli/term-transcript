//! High-level tests.

use core::num::NonZeroUsize;

use anstyle::{AnsiColor, Color, Style};
use assert_matches::assert_matches;

use super::*;
use crate::types::StyledSpan;

const SIMPLE_INPUT: &str = "[[magenta on yellow!, bold, ul]]Hello[[/]] world[[bold strike inv]]!";
const SIMPLE_STYLED: StyledStr = styled!(SIMPLE_INPUT);
const SIMPLE_STYLES: &[StyledSpan] = {
    let magenta_on_yellow = Style::new()
        .bold()
        .underline()
        .fg_color(Some(Color::Ansi(AnsiColor::Magenta)))
        .bg_color(Some(Color::Ansi(AnsiColor::BrightYellow)));

    &[
        StyledSpan {
            style: magenta_on_yellow,
            start: 0,
            len: NonZeroUsize::new(5).unwrap(),
        },
        StyledSpan {
            style: Style::new(),
            start: 5,
            len: NonZeroUsize::new(6).unwrap(),
        },
        StyledSpan {
            style: Style::new().bold().strikethrough().invert(),
            start: 11,
            len: NonZeroUsize::new(1).unwrap(),
        },
    ]
};

const fn const_eq(lhs: &str, rhs: &str) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }

    let (lhs, rhs) = (lhs.as_bytes(), rhs.as_bytes());
    let mut pos = 0;
    while pos < lhs.len() {
        if lhs[pos] != rhs[pos] {
            return false;
        }
        pos += 1;
    }
    true
}

// Check that access methods work in compile time.
const SIMPLE_STYLED_PARTS: (StyledStr, StyledStr) = {
    assert!(const_eq(SIMPLE_STYLED.text(), "Hello world!"));

    let first_span = SIMPLE_STYLED.span(0).unwrap();
    assert!(!first_span.style.is_plain());
    assert!(const_eq(first_span.text, "Hello"));

    let second_span = SIMPLE_STYLED.span_at(7).unwrap();
    assert!(const_eq(second_span.text, " world"));
    assert!(second_span.style.is_plain());

    SIMPLE_STYLED.split_at(4)
};

#[test]
fn parsing_styled_str() {
    assert_eq!(StyledStr::capacities(SIMPLE_INPUT), (12, 3));

    let styled = StackStyled::<12, 3>::parse(SIMPLE_INPUT).unwrap();
    assert_eq!(styled.text.as_str(), "Hello world!");
    assert_eq!(styled.spans.as_slice(), SIMPLE_STYLES);

    let styled: StyledString = SIMPLE_INPUT.parse().unwrap();
    assert_eq!(styled.text, "Hello world!");
    assert_eq!(styled.spans, SIMPLE_STYLES);

    assert_eq!(
        styled.to_string(),
        "[[bold underline magenta on yellow!]]Hello[[/]] world[[bold strike invert]]!"
    );
}

#[test]
fn parsing_styled_in_compile_time() {
    assert_eq!(SIMPLE_STYLED.text(), "Hello world!");
    assert_eq!(SIMPLE_STYLED.spans.as_full_slice(), SIMPLE_STYLES);
    assert_eq!(
        SIMPLE_STYLED_PARTS.0.to_string(),
        "[[bold underline magenta on yellow!]]Hell"
    );
    assert_eq!(
        SIMPLE_STYLED_PARTS.1.to_string(),
        "[[bold underline magenta on yellow!]]o[[/]] world[[bold strike invert]]!"
    );

    SIMPLE_STYLED.diff(SIMPLE_STYLED).unwrap();
}

#[test]
fn diff_by_text() {
    const EXPECTED_DIFF: StyledStr = styled!(
        "styled strings differ by text\n\
        [[bold]]Diff[[/]] [[red]]< left[[/]] / [[green]]right >[[/]] :\n\
        [[red]]<Hello world![[/]]\n\
        [[green]]>Hello[[bold green on #005f00]],[[green]] world![[/]]\n"
    );

    let other_style = styled!("Hello, [[bold green]]world[[/]]!");
    let diff = SIMPLE_STYLED.diff(other_style).unwrap_err();
    let output = StyledString::from_ansi(&diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED_DIFF);
}

#[test]
fn diff_by_style() {
    const EXPECTED_DIFF: StyledStr = styled!(
        r"styled strings differ by style
[[red]]> [[bold underline magenta on yellow!]]Hello[[/]] world[[bold strike invert]]![[/]]
[[red]]> [[white on red]]^^^^^[[/]] [[white on red]]^^^^^[[black on yellow]]![[/]]

[[bold]]Positions         Left style                Right style       [[*]]
========== ========================= =========================[[/]]
      0..5 [[bold underline magenta on yellow!]]bold underline magenta on[[/]]          (none)          [[*]]
           [[bold underline magenta on yellow!]]         yellow!         [[/]]                          [[*]]
     6..11          (none)           [[bold green]]       bold green        [[/]]
    11..12 [[bold strike invert]]   bold strike invert    [[/]]          (none)          [[*]]
"
    );

    let other_style = styled!("Hello [[bold green]]world[[/]]!");
    let diff = SIMPLE_STYLED.diff(other_style).unwrap_err();
    let output = StyledString::from_ansi(&diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED_DIFF);
}

#[test]
fn parsing_with_unstyled_ends() {
    const TEST_INPUT: &str = "test.rs: [[[bold green!]][DEBUG][[/]]] Hello";
    const STYLED: StyledStr = styled!(TEST_INPUT);

    assert_eq!(STYLED.text(), "test.rs: [[DEBUG]] Hello");
    let green = Style::new()
        .bold()
        .fg_color(Some(AnsiColor::BrightGreen.into()));
    let expected_spans = [
        SpanStr::new("test.rs: [", Style::new()),
        SpanStr::new("[DEBUG]", green),
        SpanStr::new("] Hello", Style::new()),
    ];
    assert_eq!(STYLED.spans().collect::<Vec<_>>(), expected_spans);

    let styled: StyledString = TEST_INPUT.parse().unwrap();
    assert_eq!(styled.text, "test.rs: [[DEBUG]] Hello");
    assert_eq!(styled.as_str().spans().collect::<Vec<_>>(), expected_spans);

    assert_eq!(
        styled.to_string(),
        "test.rs: [[[bold green!]][DEBUG][[/]]] Hello"
    );
}

#[test]
fn parsing_with_style_copy() {
    const STYLED: StyledStr = styled!("[[green]]Hello[[* b,i]],[[* -bold]] world[[* -fg on red]]!");

    assert_eq!(
        STYLED.to_string(),
        "[[green]]Hello[[bold italic green]],[[italic green]] world[[italic on red]]!"
    );
    assert_eq!(STYLED.spans.as_full_slice().len(), 4);
}

#[test]
fn parsing_with_no_op_style_copy() {
    const STYLED: StyledStr = styled!(
        r"[[[[*]]Brack[[*
        ]]ets!]]"
    );

    assert_eq!(STYLED.text(), "[[Brackets!]]");
    assert_eq!(STYLED.spans.as_full_slice().len(), 1);
    assert_eq!(STYLED.to_string(), "[[[[*]]Brackets!]]");
}

#[test]
fn unfinished_errors() {
    for raw in ["[[red", "[[red ", "[[red, "] {
        let err = raw.parse::<StyledString>().unwrap_err();
        assert_matches!(err.kind(), ParseErrorKind::UnfinishedStyle);
        assert_eq!(err.pos(), raw.len()..raw.len());
    }

    let raw = "[[red]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 5..6);
}

#[test]
fn bogus_delimiter_error() {
    let raw = "[[red,  , bold]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 8..9);

    let raw = "[[red,  ; bold]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 8..9);
}

#[test]
fn unfinished_background_error() {
    let raw = "[[red on, white]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnfinishedBackground);
    assert_eq!(err.pos(), 6..8);

    let raw = "[[red on]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnfinishedBackground);
    assert_eq!(err.pos(), 6..8);

    let raw = "[[red on bold]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnfinishedBackground);
    assert_eq!(err.pos(), 6..8);
}

#[test]
fn unsupported_token_error() {
    let raw = "[[what]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 2..6);

    let raw = "[[bold what]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 7..11);

    let raw = "[[bold,what]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 7..11);
}

#[test]
fn invalid_color_error() {
    let raw = "[[color(1000)]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..13);

    let raw = "[[color256]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..10);

    let raw = "[[color(001)]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..12);

    let raw = "[[color(-1)]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..11);

    let raw = "[[color(#ff)]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..12);

    let raw = "[[#cfg]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(
        err.kind(),
        ParseErrorKind::HexColor(HexColorError::InvalidHexDigit)
    );
    assert_eq!(err.pos(), 2..6);

    let raw = "[[#c0ffeg]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(
        err.kind(),
        ParseErrorKind::HexColor(HexColorError::InvalidHexDigit)
    );
    assert_eq!(err.pos(), 2..9);
}

#[test]
fn negation_errors() {
    let raw = "[[-]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 2..3);

    let raw = "[[* -]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 4..5);

    let raw = "[[* !green]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 4..10);

    let raw = "[[!bold]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::NegationWithoutCopy);
    assert_eq!(err.pos(), 2..7);

    let raw = "[[ -bold ]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::NegationWithoutCopy);
    assert_eq!(err.pos(), 3..8);

    let raw = "[[bold *]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::NonInitialCopy);
    assert_eq!(err.pos(), 7..8);
}

#[test]
fn duplicate_style_errors() {
    let raw = "[[color(3) green]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 11..16);

    let raw = "[[on green on yellow!]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 11..13);

    let raw = "[[bold green #c0ffee]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 13..20);

    let raw = "[[bold]]![[* -bold bold]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 19..23);

    let raw = "[[red]]![[* -fg green]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 16..21);
}

#[test]
fn redundant_negation_errors() {
    let raw = "[[* -bold]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::RedundantNegation);
    assert_eq!(err.pos(), 4..9);

    let raw = "[[* !fg]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::RedundantNegation);
    assert_eq!(err.pos(), 4..7);

    let raw = "[[red]]~[[* !fg]]~[[* !fg]]";
    let err = raw.parse::<StyledString>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::RedundantNegation);
    assert_eq!(err.pos(), 22..25);
}

#[test]
fn splitting_styled_string() {
    let styled = styled!("Hello, [[it green]]world[[/]]!");
    let (start, end) = styled.split_at(4);
    assert_eq!(start, styled!("Hell"));
    assert_eq!(end, styled!("o, [[it green]]world[[/]]!"));

    let (_, end) = end.split_at(3);
    assert_eq!(end, styled!("[[it green]]world[[/]]!"));

    let (start, end) = end.split_at(5);
    assert_eq!(start, styled!("[[it green]]world"));
    assert_eq!(end, styled!("!"));
}

#[test]
fn no_op_splitting() {
    let styled = styled!("Hello, [[it green]]world[[/]]!");
    let (start, end) = styled.split_at(0);
    assert_eq!(end, styled);
    assert!(start.is_empty());
    assert_eq!(start, styled!(""));

    let (start, end) = styled.split_at(styled.text().len());
    assert_eq!(start, styled);
    assert!(end.is_empty());
    assert_eq!(end, styled!(""));
}

#[test]
fn string_builder_basics() {
    let mut builder = StyledString::builder();
    builder.push_str(StyledStr::default());
    builder.push_text("\n");
    builder.push_str(StyledStr::default());
    builder.push_text("\n");
    builder.push_str(styled!("[[green]]Hello"));
    assert_eq!(builder.build(), styled!("\n\n[[green]]Hello"));
}

#[test]
fn lines_iterator() {
    let styled = styled!("\n\n[[green]]Hello");
    let lines: Vec<_> = styled.lines().collect();

    assert_eq!(
        lines,
        [
            StyledStr::default(),
            StyledStr::default(),
            styled!("[[green]]Hello")
        ]
    );
}

#[test]
fn getting_spans() {
    let styled = styled!("[[green]]Hello, [[inverted]]world[[/]]!");
    let span = styled.span(0).unwrap();
    assert_eq!(span.text, "Hello, ");
    assert_eq!(span.style, AnsiColor::Green.on_default());

    let span = styled.span(1).unwrap();
    assert_eq!(span.text, "world");
    assert_eq!(span.style, Style::new().invert());

    let span = styled.span(2).unwrap();
    assert_eq!(span.text, "!");
    assert_eq!(span.style, Style::new());

    assert_eq!(styled.span(3), None);

    let span = styled.span_at(0).unwrap();
    assert_eq!(span, SpanStr::new("Hello, ", AnsiColor::Green.on_default()));
    let span = styled.span_at(3).unwrap();
    assert_eq!(span, SpanStr::new("Hello, ", AnsiColor::Green.on_default()));
    let span = styled.span_at(7).unwrap();
    assert_eq!(span, SpanStr::new("world", Style::new().invert()));
    assert_eq!(
        styled.span_at(styled.text().len() - 1).unwrap(),
        SpanStr::plain("!")
    );
    assert_eq!(styled.span_at(styled.text().len()), None);
}

#[test]
fn slicing_string() {
    let styled = styled!("[[green]]Hello, [[inverted]]world[[/]]!");
    assert_eq!(styled.get(..).unwrap(), styled);
    assert_eq!(styled.get(..2).unwrap(), styled!("[[green]]He"));
    assert_eq!(styled.get(1..2).unwrap(), styled!("[[green]]e"));
    assert_eq!(styled.get(3..=6).unwrap(), styled!("[[green]]lo, "));
    assert_eq!(
        styled.get(..8).unwrap(),
        styled!("[[green]]Hello, [[inverted]]w")
    );
    assert_eq!(
        styled.get(1..=7).unwrap(),
        styled!("[[green]]ello, [[inverted]]w")
    );
    assert_eq!(styled.get(7..).unwrap(), styled!("[[inverted]]world[[/]]!"));
    assert_eq!(styled.get(10..).unwrap(), styled!("[[inverted]]ld[[/]]!"));
}
