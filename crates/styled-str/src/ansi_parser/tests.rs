use super::*;
use crate::{styled, types::StyledStr};

#[test]
fn term_roundtrip_simple() {
    const STYLED: StyledStr = styled!("Hello, [[bold green]]world[[]]!");

    let ansi = STYLED.ansi().to_string();
    let restored = StyledString::from_ansi(&ansi).unwrap();
    assert_eq!(STYLED, restored);
}

#[test]
fn term_roundtrip_with_multiple_colors() {
    const STYLED: StyledStr = styled!(
        "He[[black on white]]ll[[magenta]]o [[i s, green on yellow]]world[[ul dim on cyan]]!"
    );

    let ansi = STYLED.ansi().to_string();
    let restored = StyledString::from_ansi(&ansi).unwrap();
    assert_eq!(STYLED, restored);

    assert_eq!(
        STYLED.to_string(),
        "He[[black on white]]ll[[magenta]]o [[italic strike green on yellow]]world[[dim underline on cyan]]!"
    );
}

#[test]
fn roundtrip_with_indexed_colors() {
    const STYLED: StyledStr = styled!("H[[5]]e[[on 11]]l[[33]]l[[on 250]]o");

    let ansi = STYLED.ansi().to_string();
    let restored = StyledString::from_ansi(&ansi).unwrap();
    assert_eq!(STYLED, restored);

    assert_eq!(
        STYLED.to_string(),
        "H[[magenta]]e[[on yellow!]]l[[#0087ff]]l[[on #bcbcbc]]o"
    );
}

#[test]
fn roundtrip_with_rgb_colors() {
    const STYLED: StyledStr = styled!("H[[#101e2f]]e[[on #fffefd]]l[[#000]]l[[#00a080]]o");

    let ansi = STYLED.ansi().to_string();
    let restored = StyledString::from_ansi(&ansi).unwrap();
    assert_eq!(STYLED, restored);

    assert_eq!(
        STYLED.to_string(),
        "H[[#101e2f]]e[[on #fffefd]]l[[#000]]l[[#00a080]]o"
    );
}

#[test]
fn skipping_ocs_sequence_with_bell_terminator() {
    let term_output = "\u{1b}]0;C:\\WINDOWS\\system32\\cmd.EXE\u{7}echo foo";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.text, "echo foo");
    assert_eq!(parsed.spans.len(), 1);
}

#[test]
fn skipping_ocs_sequence_with_st_terminator() {
    let term_output = "\u{1b}]0;C:\\WINDOWS\\system32\\cmd.EXE\u{1b}\\echo foo";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.text, "echo foo");
    assert_eq!(parsed.spans.len(), 1);
}

#[test]
fn skipping_non_color_csi_sequence() {
    let term_output = "\u{1b}[49Xecho foo";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.text, "echo foo");
    assert_eq!(parsed.spans.len(), 1);
}

#[test]
fn implicit_reset_sequence() {
    let term_output = "\u{1b}[34mblue\u{1b}[m";
    let parsed = StyledString::from_ansi(term_output).unwrap();

    assert_eq!(parsed.ansi().to_string(), "\u{1b}[34mblue\u{1b}[0m");
}

#[test]
fn intense_color() {
    let term_output = "\u{1b}[94mblue\u{1b}[m";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.ansi().to_string(), "\u{1b}[94mblue\u{1b}[0m");

    let term_output = "\u{1b}[38;5;12mblue\u{1b}[m";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.ansi().to_string(), "\u{1b}[94mblue\u{1b}[0m");
}

#[test]
fn carriage_return_at_end_of_line() {
    let term_output = "\u{1b}[32mgreen\u{1b}[m\r";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.ansi().to_string(), "\u{1b}[32mgreen\u{1b}[0m");
}

#[test]
fn carriage_return_at_end_of_line_with_style_afterwards() {
    let term_output = "\u{1b}[32mgreen\u{1b}[m!\r\u{1b}[m";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.ansi().to_string(), "\u{1b}[32mgreen\u{1b}[0m!");
}

#[test]
fn carriage_return_at_middle_of_line() {
    let term_output = "\u{1b}[32mgreen\u{1b}[m\r\u{1b}[34mblue\u{1b}[m";
    let parsed = StyledString::from_ansi(term_output).unwrap();
    assert_eq!(parsed.ansi().to_string(), "\u{1b}[34mblue\u{1b}[0m");
}
