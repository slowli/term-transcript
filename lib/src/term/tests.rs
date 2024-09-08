use std::io::Write;

use termcolor::{Ansi, Color, ColorSpec, WriteColor};

use super::*;

fn prepare_term_output() -> anyhow::Result<String> {
    let mut writer = Ansi::new(vec![]);
    writer.set_color(
        ColorSpec::new()
            .set_fg(Some(Color::Cyan))
            .set_underline(true),
    )?;
    write!(writer, "Hello")?;
    writer.reset()?;
    write!(writer, ", ")?;
    writer.set_color(
        ColorSpec::new()
            .set_fg(Some(Color::White))
            .set_bg(Some(Color::Green))
            .set_intense(true),
    )?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    String::from_utf8(writer.into_inner()).map_err(From::from)
}

#[test]
fn converting_captured_output_to_text() -> anyhow::Result<()> {
    let output = Captured(prepare_term_output()?);
    assert_eq!(output.to_plaintext()?, "Hello, world!");
    Ok(())
}

#[test]
fn converting_captured_output_to_html() -> anyhow::Result<()> {
    const EXPECTED_HTML: &str = "<span class=\"underline fg6\">Hello</span>, \
        <span class=\"fg15 bg10\">world</span>!";

    let output = Captured(prepare_term_output()?);
    assert_eq!(output.to_html()?, EXPECTED_HTML);
    Ok(())
}

fn assert_eq_term_output(actual: &[u8], expected: &[u8]) {
    assert_eq!(
        String::from_utf8_lossy(actual),
        String::from_utf8_lossy(expected)
    );
}

#[test]
fn term_roundtrip_simple() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![]);
    write!(writer, "Hello, ")?;
    writer.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Green)))?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn term_roundtrip_with_multiple_colors() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![]);
    write!(writer, "He")?;
    writer.set_color(
        ColorSpec::new()
            .set_bg(Some(Color::White))
            .set_fg(Some(Color::Black)),
    )?;
    write!(writer, "ll")?;
    writer.set_color(
        ColorSpec::new()
            .set_intense(true)
            .set_fg(Some(Color::Magenta)),
    )?;
    write!(writer, "o")?;
    writer.set_color(
        ColorSpec::new()
            .set_italic(true)
            .set_fg(Some(Color::Green))
            .set_bg(Some(Color::Yellow)),
    )?;
    write!(writer, "world")?;
    writer.set_color(
        ColorSpec::new()
            .set_underline(true)
            .set_dimmed(true)
            .set_bg(Some(Color::Cyan)),
    )?;
    write!(writer, "!")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn roundtrip_with_indexed_colors() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![]);
    write!(writer, "H")?;
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(5))))?;
    write!(writer, "e")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(11))))?;
    write!(writer, "l")?;
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(33))))?;
    write!(writer, "l")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(250))))?;
    write!(writer, "o")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn roundtrip_with_rgb_colors() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![]);
    write!(writer, "H")?;
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(16, 22, 35))))?;
    write!(writer, "e")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Rgb(255, 254, 253))))?;
    write!(writer, "l")?;
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(0, 0, 0))))?;
    write!(writer, "l")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Rgb(0, 160, 128))))?;
    write!(writer, "o")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn skipping_ocs_sequence_with_bell_terminator() -> anyhow::Result<()> {
    let term_output = "\u{1b}]0;C:\\WINDOWS\\system32\\cmd.EXE\u{7}echo foo";

    let mut writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(String::from_utf8(rendered_output)?, "echo foo");
    Ok(())
}

#[test]
fn skipping_ocs_sequence_with_st_terminator() -> anyhow::Result<()> {
    let term_output = "\u{1b}]0;C:\\WINDOWS\\system32\\cmd.EXE\u{1b}\\echo foo";

    let mut writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(String::from_utf8(rendered_output)?, "echo foo");
    Ok(())
}

#[test]
fn skipping_non_color_csi_sequence() -> anyhow::Result<()> {
    let term_output = "\u{1b}[49Xecho foo";

    let mut writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(String::from_utf8(rendered_output)?, "echo foo");
    Ok(())
}

#[test]
fn implicit_reset_sequence() -> anyhow::Result<()> {
    let term_output = "\u{1b}[34mblue\u{1b}[m";

    let mut writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[34mblue\u{1b}[0m"
    );
    Ok(())
}

#[test]
fn intense_color() -> anyhow::Result<()> {
    let term_output = "\u{1b}[94mblue\u{1b}[m";

    let mut writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[38;5;12mblue\u{1b}[0m"
    );
    Ok(())
}

#[test]
fn carriage_return_at_middle_of_line() -> anyhow::Result<()> {
    let term_output = "\u{1b}[32mgreen\u{1b}[m\r\u{1b}[34mblue\u{1b}[m";

    let mut writer = Ansi::new(vec![]);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[34mblue\u{1b}[0m"
    );
    Ok(())
}
