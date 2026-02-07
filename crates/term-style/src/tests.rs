//! High-level tests.

use anstyle::{AnsiColor, Color, Style};
use assert_matches::assert_matches;

use super::*;

const SIMPLE_INPUT: &str = "[[magenta on yellow*, bold, ul]]Hello[[]] world[[bold strike inv]]!";
const SIMPLE_STYLED: Styled = styled!(SIMPLE_INPUT);
const SIMPLE_STYLES: &[StyledSpan] = &[
    StyledSpan {
        style: Style::new()
            .bold()
            .underline()
            .fg_color(Some(Color::Ansi(AnsiColor::Magenta)))
            .bg_color(Some(Color::Ansi(AnsiColor::BrightYellow))),
        len: 5,
    },
    StyledSpan {
        style: Style::new(),
        len: 6,
    },
    StyledSpan {
        style: Style::new().bold().strikethrough().invert(),
        len: 1,
    },
];

#[test]
fn parsing_styled_str() {
    assert_eq!(Styled::capacities(SIMPLE_INPUT), (12, 3));

    let styled = StackStyled::<12, 3>::parse(SIMPLE_INPUT).unwrap();
    assert_eq!(styled.text.as_str(), "Hello world!");
    assert_eq!(styled.spans.as_slice(), SIMPLE_STYLES);

    let styled: DynStyled = SIMPLE_INPUT.parse().unwrap();
    assert_eq!(styled.text, "Hello world!");
    assert_eq!(styled.spans, SIMPLE_STYLES);

    assert_eq!(
        styled.to_string(),
        "[[bold underline magenta on yellow*]]Hello[[]] world[[bold strike invert]]!"
    );
}

#[test]
fn parsing_styled_in_compile_time() {
    assert_eq!(SIMPLE_STYLED.text(), "Hello world!");
    assert_eq!(SIMPLE_STYLED.spans(), SIMPLE_STYLES);

    SIMPLE_STYLED.diff(&SIMPLE_STYLED).unwrap();
}

#[test]
fn diff_by_text() {
    const EXPECTED_DIFF: Styled = styled!(
        "Styled strings differ by text\n\
        [[bold]]Diff[[]] [[red]]< left[[]] / [[green]]right >[[]] :\n\
        [[red]]<Hello world![[]]\n\
        [[green]]>Hello[[bold green on #005f00]],[[green]] world![[]]\n"
    );

    let other_style = styled!("Hello, [[bold green]]world[[]]!");
    let diff = SIMPLE_STYLED.diff(&other_style).unwrap_err();
    let output = DynStyled::from_ansi(&diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED_DIFF);
}

#[test]
fn diff_by_style() {
    const EXPECTED_DIFF: Styled = styled!(
        r"Styled strings differ by style
[[red]]> [[bold underline magenta on yellow*]]Hello[[]] world[[bold strike invert]]![[]]
[[red]]> [[white on red]]^^^^^[[]] [[white on red]]^^^^^[[black on yellow]]![[]]

[[bold]]Positions         Left style                Right style       [[*]]
========== ========================= =========================[[]]
      0..5 [[bold underline magenta on yellow*]]bold underline magenta on[[]]          (none)          [[*]]
           [[bold underline magenta on yellow*]]         yellow*         [[]]                          [[*]]
     6..11          (none)           [[bold green]]       bold green        [[]]
    11..12 [[bold strike invert]]   bold strike invert    [[]]          (none)          [[*]]
"
    );

    let other_style = styled!("Hello [[bold green]]world[[]]!");
    let diff = SIMPLE_STYLED.diff(&other_style).unwrap_err();
    let output = DynStyled::from_ansi(&diff.to_string()).unwrap();
    assert_eq!(output, EXPECTED_DIFF);
}

#[test]
fn parsing_with_unstyled_ends() {
    const TEST_INPUT: &str = "test.rs: [[[bold green*]][DEBUG][[]]] Hello";
    const STYLED: Styled = styled!(TEST_INPUT);

    assert_eq!(STYLED.text(), "test.rs: [[DEBUG]] Hello");
    let expected_spans = [
        StyledSpan {
            style: Style::new(),
            len: 10,
        },
        StyledSpan {
            style: Style::new()
                .bold()
                .fg_color(Some(AnsiColor::BrightGreen.into())),
            len: 7,
        },
        StyledSpan {
            style: Style::new(),
            len: 7,
        },
    ];
    assert_eq!(STYLED.spans(), expected_spans);

    let styled: DynStyled = TEST_INPUT.parse().unwrap();
    assert_eq!(styled.text, "test.rs: [[DEBUG]] Hello");
    assert_eq!(styled.spans, expected_spans);

    assert_eq!(
        styled.to_string(),
        "test.rs: [[[bold green*]][DEBUG][[]]] Hello"
    );
}

#[test]
fn parsing_with_style_copy() {
    const STYLED: Styled = styled!("[[green]]Hello[[* b,i]],[[* -bold]] world[[* -fg on red]]!");

    assert_eq!(
        STYLED.to_string(),
        "[[green]]Hello[[bold italic green]],[[italic green]] world[[italic on red]]!"
    );
    assert_eq!(STYLED.spans().len(), 4);
}

#[test]
fn parsing_with_no_op_style_copy() {
    const STYLED: Styled = styled!(
        r"[[[[*]]Brack[[*
        ]]ets!]]"
    );

    assert_eq!(STYLED.text(), "[[Brackets!]]");
    assert_eq!(STYLED.spans().len(), 1);
    assert_eq!(STYLED.to_string(), "[[[[*]]Brackets!]]");
}

#[test]
fn unfinished_errors() {
    for raw in ["[[red", "[[red ", "[[red, "] {
        let err = raw.parse::<DynStyled>().unwrap_err();
        assert_matches!(err.kind(), ParseErrorKind::UnfinishedStyle);
        assert_eq!(err.pos(), raw.len()..raw.len());
    }

    let raw = "[[red]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 5..6);
}

#[test]
fn bogus_delimiter_error() {
    let raw = "[[red,  , bold]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 8..9);

    let raw = "[[red,  ; bold]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 8..9);
}

#[test]
fn unfinished_background_error() {
    let raw = "[[red on, white]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnfinishedBackground);
    assert_eq!(err.pos(), 6..8);

    let raw = "[[red on]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnfinishedBackground);
    assert_eq!(err.pos(), 6..8);

    let raw = "[[red on bold]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnfinishedBackground);
    assert_eq!(err.pos(), 6..8);
}

#[test]
fn unsupported_token_error() {
    let raw = "[[what]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 2..6);

    let raw = "[[bold what]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 7..11);

    let raw = "[[bold,what]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 7..11);
}

#[test]
fn invalid_color_error() {
    let raw = "[[1000]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..6);

    let raw = "[[256]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..5);

    let raw = "[[001]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidIndexColor);
    assert_eq!(err.pos(), 2..5);

    let raw = "[[-1]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 2..4);

    let raw = "[[#cfg]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(
        err.kind(),
        ParseErrorKind::HexColor(HexColorError::InvalidHexDigit)
    );
    assert_eq!(err.pos(), 2..6);

    let raw = "[[#c0ffeg]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(
        err.kind(),
        ParseErrorKind::HexColor(HexColorError::InvalidHexDigit)
    );
    assert_eq!(err.pos(), 2..9);
}

#[test]
fn negation_errors() {
    let raw = "[[-]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 2..3);

    let raw = "[[* -]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 4..5);

    let raw = "[[* !green]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedEffect);
    assert_eq!(err.pos(), 4..10);

    let raw = "[[!bold]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::NegationWithoutCopy);
    assert_eq!(err.pos(), 2..7);

    let raw = "[[ -bold ]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::NegationWithoutCopy);
    assert_eq!(err.pos(), 3..8);

    let raw = "[[bold *]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::NonInitialCopy);
    assert_eq!(err.pos(), 7..8);
}

#[test]
fn duplicate_style_errors() {
    let raw = "[[3 green]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 4..9);

    let raw = "[[on green on yellow*]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 11..13);

    let raw = "[[bold green #c0ffee]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 13..20);

    let raw = "[[bold]]![[* -bold bold]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 19..23);

    let raw = "[[red]]![[* -fg green]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::DuplicateSpecifier);
    assert_eq!(err.pos(), 16..21);
}

#[test]
fn redundant_negation_errors() {
    let raw = "[[* -bold]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::RedundantNegation);
    assert_eq!(err.pos(), 4..9);

    let raw = "[[* !fg]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::RedundantNegation);
    assert_eq!(err.pos(), 4..7);

    let raw = "[[red]]~[[* !fg]]~[[* !fg]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::RedundantNegation);
    assert_eq!(err.pos(), 22..25);
}
