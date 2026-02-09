use assert_matches::assert_matches;

use super::*;

#[test]
fn tokenization_works() {
    let mut cursor = StrCursor::new("bold, ul");
    let token = cursor.take_token();
    assert_eq!(token, 0..4);
    assert_eq!(cursor.pos(), 4);

    let mut cursor = StrCursor::new("bold");
    let token = cursor.take_token();
    assert_eq!(token, 0..4);
    assert_eq!(cursor.pos(), 4);

    let mut cursor = StrCursor::new("bold]]");
    let token = cursor.take_token();
    assert_eq!(token, 0..4);
    assert_eq!(cursor.pos(), 4);
}

#[test]
fn parsing_style() {
    let mut cursor = StrCursor::new("bold, ul magenta on yellow!");
    let style = cursor.parse_style(&Style::new(), false).unwrap();
    let expected_style = Style::new()
        .bold()
        .underline()
        .fg_color(Some(AnsiColor::Magenta.into()))
        .bg_color(Some(AnsiColor::BrightYellow.into()));
    assert_eq!(style, expected_style);
    assert!(cursor.is_eof(), "{cursor:?}");
}

#[test]
fn parsing_style_with_complex_colors() {
    let mut cursor = StrCursor::new("dim i invert; blink; 42 on #c0ffee]]");
    let style = cursor.parse_style(&Style::new(), true).unwrap();
    let expected_style = Style::new()
        .dimmed()
        .blink()
        .invert()
        .italic()
        .fg_color(Some(RgbColor(0, 215, 135).into()))
        .bg_color(Some(RgbColor(0xc0, 0xff, 0xee).into()));
    assert_eq!(style, expected_style);
    assert!(cursor.is_eof(), "{cursor:?}");
}

#[test]
fn standalone_style_parsing() {
    let style = RichStyle::parse("red on 7, bold", &Style::new()).unwrap();
    assert_eq!(
        style,
        Style::new()
            .bold()
            .fg_color(Some(AnsiColor::Red.into()))
            .bg_color(Some(AnsiColor::White.into()))
    );

    let err = RichStyle::parse("red on 7]], bold", &Style::new()).unwrap_err();
    assert_matches!(err.kind(), ParseErrorKind::BogusDelimiter);
    assert_eq!(err.pos(), 8..9);
}

#[test]
fn escaping_text() {
    assert_eq!(EscapedText("test: [OK]").to_string(), "test: [OK]");

    assert_eq!(EscapedText("test: [[OK]]").to_string(), "test: [[[[*]]OK]]");

    assert_eq!(
        EscapedText("[[OK]] test :[[[").to_string(),
        "[[[[*]]OK]] test :[[[[[*]]"
    );
}
