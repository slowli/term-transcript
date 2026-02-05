//! High-level tests.

use anstyle::{AnsiColor, Color, Style};

use super::*;

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
    let raw = "[[magenta on yellow*, bold, ul]]Hello[[]] world[[bold strike inv]]!";
    assert_eq!(Styled::capacities(raw), (12, 3));
    let styled = StackStyled::<12, 3>::parse(raw).unwrap();
    assert_eq!(styled.text.as_str(), "Hello world!");
    assert_eq!(styled.spans.as_slice(), SIMPLE_STYLES);
}

#[test]
fn parsing_styled_in_compile_time() {
    const STYLED: Styled =
        styled!("[[magenta on yellow*, bold, ul]]Hello[[]] world[[bold strike inv]]!");

    assert_eq!(STYLED.text(), "Hello world!");
    assert_eq!(STYLED.spans(), SIMPLE_STYLES);
}

#[test]
fn parsing_with_unstyled_ends() {
    const STYLED: Styled = styled!("test.rs: [[bold green*]][DEBUG][[]] Hello");

    assert_eq!(STYLED.text(), "test.rs: [[DEBUG]] Hello");
    assert_eq!(
        STYLED.spans(),
        [
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
        ]
    );
}
