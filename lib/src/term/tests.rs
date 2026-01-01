use super::*;
use crate::style::{Ansi, Color, RgbColor, Style, WriteStyled};

fn prepare_term_output() -> anyhow::Result<String> {
    let mut writer = Ansi::new(vec![], true);
    writer.write_style(&Style {
        underline: true,
        fg: Some(Color::CYAN),
        ..Style::default()
    })?;
    write!(writer, "Hello")?;
    writer.reset()?;
    write!(writer, ", ")?;
    writer.write_style(&Style {
        fg: Some(Color::INTENSE_WHITE),
        bg: Some(Color::INTENSE_GREEN),
        ..Style::default()
    })?;
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

fn assert_eq_term_output(actual: &[u8], expected: &[u8]) {
    assert_eq!(
        String::from_utf8_lossy(actual),
        String::from_utf8_lossy(expected)
    );
}

#[test]
fn term_roundtrip_simple() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![], true);
    write!(writer, "Hello, ")?;
    writer.write_style(&Style {
        bold: true,
        fg: Some(Color::GREEN),
        ..Style::default()
    })?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn term_roundtrip_with_multiple_colors() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![], true);
    write!(writer, "He")?;
    writer.write_style(&Style {
        fg: Some(Color::BLACK),
        bg: Some(Color::WHITE),
        ..Style::default()
    })?;
    write!(writer, "ll")?;
    writer.write_style(&Style {
        fg: Some(Color::MAGENTA),
        ..Style::default()
    })?;
    write!(writer, "o")?;
    writer.write_style(&Style {
        italic: true,
        fg: Some(Color::GREEN),
        bg: Some(Color::YELLOW),
        ..Style::default()
    })?;
    write!(writer, "world")?;
    writer.write_style(&Style {
        underline: true,
        dimmed: true,
        bg: Some(Color::CYAN),
        ..Style::default()
    })?;
    write!(writer, "!")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn roundtrip_with_indexed_colors() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![], true);
    write!(writer, "H")?;
    writer.write_style(&Style {
        fg: Some(Color::Index(5)),
        ..Style::default()
    })?;
    write!(writer, "e")?;
    writer.write_style(&Style {
        bg: Some(Color::Index(11)),
        ..Style::default()
    })?;
    write!(writer, "l")?;
    writer.write_style(&Style {
        fg: Some(Color::Index(33)),
        ..Style::default()
    })?;
    write!(writer, "l")?;
    writer.write_style(&Style {
        bg: Some(Color::Index(250)),
        ..Style::default()
    })?;
    write!(writer, "o")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn roundtrip_with_rgb_colors() -> anyhow::Result<()> {
    let mut writer = Ansi::new(vec![], true);
    write!(writer, "H")?;
    writer.write_style(&Style {
        fg: Some(Color::Rgb(RgbColor(16, 22, 35))),
        ..Style::default()
    })?;
    write!(writer, "e")?;
    writer.write_style(&Style {
        bg: Some(Color::Rgb(RgbColor(255, 254, 253))),
        ..Style::default()
    })?;
    write!(writer, "l")?;
    writer.write_style(&Style {
        fg: Some(Color::Rgb(RgbColor(0, 0, 0))),
        ..Style::default()
    })?;
    write!(writer, "l")?;
    writer.write_style(&Style {
        bg: Some(Color::Rgb(RgbColor(0, 160, 128))),
        ..Style::default()
    })?;
    write!(writer, "o")?;

    let term_output = writer.into_inner();

    let mut new_writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut new_writer).parse(&term_output)?;
    let new_term_output = new_writer.into_inner();
    assert_eq_term_output(&new_term_output, &term_output);
    Ok(())
}

#[test]
fn skipping_ocs_sequence_with_bell_terminator() -> anyhow::Result<()> {
    let term_output = "\u{1b}]0;C:\\WINDOWS\\system32\\cmd.EXE\u{7}echo foo";

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(String::from_utf8(rendered_output)?, "echo foo");
    Ok(())
}

#[test]
fn skipping_ocs_sequence_with_st_terminator() -> anyhow::Result<()> {
    let term_output = "\u{1b}]0;C:\\WINDOWS\\system32\\cmd.EXE\u{1b}\\echo foo";

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(String::from_utf8(rendered_output)?, "echo foo");
    Ok(())
}

#[test]
fn skipping_non_color_csi_sequence() -> anyhow::Result<()> {
    let term_output = "\u{1b}[49Xecho foo";

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(String::from_utf8(rendered_output)?, "echo foo");
    Ok(())
}

#[test]
fn implicit_reset_sequence() -> anyhow::Result<()> {
    let term_output = "\u{1b}[34mblue\u{1b}[m";

    let mut writer = Ansi::new(vec![], true);
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

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[38;5;12mblue\u{1b}[0m"
    );
    Ok(())
}

#[test]
fn carriage_return_at_end_of_line() -> anyhow::Result<()> {
    let term_output = "\u{1b}[32mgreen\u{1b}[m\r";

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[32mgreen\u{1b}[0m"
    );
    Ok(())
}

#[test]
fn carriage_return_at_end_of_line_with_style_afterwards() -> anyhow::Result<()> {
    let term_output = "\u{1b}[32mgreen\u{1b}[m!\r\u{1b}[m";

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[32mgreen\u{1b}[0m!"
    );
    Ok(())
}

#[test]
fn carriage_return_at_middle_of_line() -> anyhow::Result<()> {
    let term_output = "\u{1b}[32mgreen\u{1b}[m\r\u{1b}[34mblue\u{1b}[m";

    let mut writer = Ansi::new(vec![], true);
    TermOutputParser::new(&mut writer).parse(term_output.as_bytes())?;
    let rendered_output = writer.into_inner();

    assert_eq!(
        String::from_utf8(rendered_output)?,
        "\u{1b}[0m\u{1b}[34mblue\u{1b}[0m"
    );
    Ok(())
}
