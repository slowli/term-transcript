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
    assert_matches!(err.kind(), ParseErrorKind::UnsupportedStyle);
    assert_eq!(err.pos(), 2..4);

    let raw = "[[#cfg]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidHexColor);
    assert_eq!(err.pos(), 2..6);

    let raw = "[[#c0ffeg]]";
    let err = raw.parse::<DynStyled>().unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::InvalidHexColor);
    assert_eq!(err.pos(), 2..9);
}
