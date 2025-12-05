use std::io::Write;

use termcolor::WriteColor;

use super::*;

type HtmlWriter = GenericWriter<HtmlLine>;

#[test]
fn html_escaping() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(None);
    write!(writer, "1 < 2 && 4 >= 3")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].html, "1 &lt; 2 &amp;&amp; 4 &gt;= 3");
    Ok(())
}

#[test]
fn html_writer_basic_colors() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(None);
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

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].html,
        r#"Hello, <span class="bold underline fg2 bg7">world</span>!"#
    );

    Ok(())
}

#[test]
fn html_writer_intense_color() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(None);

    writer.set_color(ColorSpec::new().set_intense(true).set_fg(Some(Color::Blue)))?;
    write!(writer, "blue")?;
    writer.reset()?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].html, r#"<span class="fg12">blue</span>"#);
    Ok(())
}

#[test]
fn html_writer_embedded_spans_with_reset() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(None);
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

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].html,
        "<span class=\"dimmed fg2 bg7\">Hello, </span><span class=\"fg3\">world</span>!"
    );

    Ok(())
}

#[test]
fn html_writer_custom_colors() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(None);
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

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].html,
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
        Line { text: "tex t", br: Some(LineBreak::Hard), char_width: 5 },
        Line { text: "ext ", br: Some(LineBreak::Hard), char_width: 4 },
        Line { text: "\u{7d75}\u{6587}", br: Some(LineBreak::Hard), char_width: 4 },
        Line { text: "\u{5b57}", br: None, char_width: 2 },
        Line { text: "\u{1f602}\u{1f602}", br: Some(LineBreak::Hard), char_width: 4 },
        Line { text: "\u{1f602}", br: None, char_width: 2 },
        Line { text: "", br: None, char_width: 0 },
    ];
    assert_eq!(lines, expected_lines);
}

#[test]
fn splitting_lines_in_writer() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(Some(5));

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

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].html, "Hello");
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(
        lines[1].html,
        ", <span class=\"bold underline fg2 bg7\">wor</span>"
    );
    assert_eq!(lines[1].br, Some(LineBreak::Hard));
    assert_eq!(
        lines[2].html,
        "<span class=\"bold underline fg2 bg7\">ld</span>! M"
    );
    assert_eq!(lines[2].br, Some(LineBreak::Hard));
    assert_eq!(lines[3].html, "ore&gt;");
    assert_eq!(lines[3].br, None);
    assert_eq!(lines[4].html, "text");
    assert_eq!(lines[4].br, None);

    Ok(())
}

#[test]
fn splitting_lines_with_escaped_chars() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(Some(5));
    writeln!(writer, ">>>>>>>")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].html, "&gt;&gt;&gt;&gt;&gt;");
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(lines[1].html, "&gt;&gt;");

    let mut writer = HtmlWriter::new(Some(5));
    for _ in 0..7 {
        write!(writer, ">")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].html, "&gt;&gt;&gt;&gt;&gt;");
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(lines[1].html, "&gt;&gt;");
    Ok(())
}

#[test]
fn splitting_lines_with_newlines() -> anyhow::Result<()> {
    let mut writer = HtmlWriter::new(Some(5));

    for _ in 0..2 {
        writeln!(writer, "< test >")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].html, "&lt; tes");
    assert_eq!(lines[1].html, "t &gt;");
    assert_eq!(lines[2].html, "&lt; tes");
    assert_eq!(lines[3].html, "t &gt;");

    let mut writer = HtmlWriter::new(Some(5));
    for _ in 0..2 {
        writeln!(writer, "<< test >>")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].html, "&lt;&lt; te");
    assert_eq!(lines[1].html, "st &gt;&gt;");
    assert_eq!(lines[2].html, "&lt;&lt; te");
    assert_eq!(lines[3].html, "st &gt;&gt;");
    Ok(())
}
