use super::*;

use std::io::Write;

#[test]
fn html_escaping() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, None);
    write!(writer, "1 < 2 && 4 >= 3")?;

    assert_eq!(buffer, "1 &lt; 2 &amp;&amp; 4 &gt;= 3");
    Ok(())
}

#[test]
fn html_writer_basic_colors() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, None);
    write!(writer, "Hello, ")?;
    writer.set_color(
        ColorSpec::new()
            .set_bold(true)
            .set_underline(true)
            .set_fg(Some(Color::Green))
            .set_bg(Some(Color::White)),
    )?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    assert_eq!(
        buffer,
        r#"Hello, <span class="bold underline fg2 bg7">world</span>!"#
    );

    Ok(())
}

#[test]
fn html_writer_intense_color() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, None);

    writer.set_color(ColorSpec::new().set_intense(true).set_fg(Some(Color::Blue)))?;
    write!(writer, "blue")?;
    writer.reset()?;

    assert_eq!(buffer, r#"<span class="fg12">blue</span>"#);
    Ok(())
}

#[test]
fn html_writer_embedded_spans_with_reset() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, None);
    writer.set_color(
        ColorSpec::new()
            .set_dimmed(true)
            .set_fg(Some(Color::Green))
            .set_bg(Some(Color::White)),
    )?;
    write!(writer, "Hello, ")?;
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    assert_eq!(
        buffer,
        "<span class=\"dimmed fg2 bg7\">Hello, </span><span class=\"fg3\">world</span>!"
    );

    Ok(())
}

#[test]
fn html_writer_custom_colors() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, None);
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(5))))?;
    write!(writer, "H")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(14))))?;
    write!(writer, "e")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(76))))?;
    write!(writer, "l")?;
    writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(200))))?;
    write!(writer, "l")?;
    writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(250))))?;
    write!(writer, "o")?;
    writer.reset()?;

    assert_eq!(
        buffer,
        "<span class=\"fg5\">H</span>\
             <span class=\"bg14\">e</span>\
             <span style=\"background: #5fd700;\">l</span>\
             <span style=\"color: #ff00d7;\">l</span>\
             <span style=\"background: #bcbcbc;\">o</span>"
    );
    Ok(())
}

#[test]
fn splitting_lines() {
    let mut splitter = LineSplitter::new(5);
    let lines =
        splitter.split_lines("tex text \u{7d75}\u{6587}\u{5b57}\n\u{1f602}\u{1f602}\u{1f602}\n");

    #[rustfmt::skip]
        let expected_lines = vec![
        Line { text: "tex t", br: Some(LineBreak::Hard) },
        Line { text: "ext ", br: Some(LineBreak::Hard) },
        Line { text: "\u{7d75}\u{6587}", br: Some(LineBreak::Hard) },
        Line { text: "\u{5b57}", br: None },
        Line { text: "\u{1f602}\u{1f602}", br: Some(LineBreak::Hard) },
        Line { text: "\u{1f602}", br: None },
        Line { text: "", br: None },
    ];
    assert_eq!(lines, expected_lines);
}

#[test]
fn slitting_lines_in_writer() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, Some(5));

    write!(writer, "Hello, ")?;
    writer.set_color(
        ColorSpec::new()
            .set_bold(true)
            .set_underline(true)
            .set_fg(Some(Color::Green))
            .set_bg(Some(Color::White)),
    )?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "! More>\ntext")?;

    assert_eq!(
        buffer,
        "Hello<b class=\"hard-br\"><br/></b>, <span class=\"bold underline fg2 bg7\">\
             wor<b class=\"hard-br\"><br/></b>ld</span>! \
             M<b class=\"hard-br\"><br/></b>ore&gt;\ntext"
    );
    Ok(())
}

#[test]
fn splitting_lines_with_escaped_chars() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, Some(5));

    writeln!(writer, ">>>>>>>")?;
    assert_eq!(
        buffer,
        "&gt;&gt;&gt;&gt;&gt;<b class=\"hard-br\"><br/></b>&gt;&gt;\n"
    );

    {
        buffer.clear();
        let mut writer = HtmlWriter::new(&mut buffer, Some(5));
        for _ in 0..7 {
            write!(writer, ">")?;
        }
        assert_eq!(
            buffer,
            "&gt;&gt;&gt;&gt;&gt;<b class=\"hard-br\"><br/></b>&gt;&gt;"
        );
    }
    Ok(())
}

#[test]
fn splitting_lines_with_newlines() -> anyhow::Result<()> {
    let mut buffer = String::new();
    let mut writer = HtmlWriter::new(&mut buffer, Some(5));

    for _ in 0..2 {
        writeln!(writer, "< test >")?;
    }
    assert_eq!(
        buffer,
        "&lt; tes<b class=\"hard-br\"><br/></b>t &gt;\n&lt; \
             tes<b class=\"hard-br\"><br/></b>t &gt;\n"
    );

    buffer.clear();
    let mut writer = HtmlWriter::new(&mut buffer, Some(5));
    for _ in 0..2 {
        writeln!(writer, "<< test >>")?;
    }
    assert_eq!(
        buffer,
        "&lt;&lt; te<b class=\"hard-br\"><br/></b>st &gt;&gt;\n\
             &lt;&lt; te<b class=\"hard-br\"><br/></b>st &gt;&gt;\n"
    );
    Ok(())
}
