use super::*;
use crate::style::Color;

impl From<&str> for StyledSpan {
    fn from(text: &str) -> Self {
        Self {
            style: Style::default(),
            text: text.into(),
        }
    }
}

#[test]
fn html_writer_basic_colors() -> anyhow::Result<()> {
    let mut writer = LineWriter::new(None);
    write!(writer, "Hello, ")?;
    writer.write_style(&Style {
        bold: true,
        underline: true,
        fg: Some(Color::GREEN),
        bg: Some(Color::WHITE),
        ..Style::default()
    })?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].spans,
        [
            "Hello, ".into(),
            StyledSpan {
                style: Style {
                    bold: true,
                    underline: true,
                    fg: Some(Color::Index(2)),
                    bg: Some(Color::Index(7)),
                    ..Style::default()
                },
                text: "world".into(),
            },
            "!".into(),
        ]
    );

    Ok(())
}

#[test]
fn html_writer_intense_color() -> anyhow::Result<()> {
    let mut writer = LineWriter::new(None);

    writer.write_style(&Style {
        fg: Some(Color::INTENSE_BLUE),
        ..Style::default()
    })?;
    write!(writer, "blue")?;
    writer.reset()?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].spans,
        [StyledSpan {
            style: Style {
                fg: Some(Color::Index(12)),
                ..Style::default()
            },
            text: "blue".into(),
        }]
    );
    Ok(())
}

#[test]
fn html_writer_embedded_spans_with_reset() -> anyhow::Result<()> {
    let mut writer = LineWriter::new(None);
    writer.write_style(&Style {
        dimmed: true,
        fg: Some(Color::GREEN),
        bg: Some(Color::WHITE),
        ..Style::default()
    })?;
    write!(writer, "Hello, ")?;
    writer.write_style(&Style {
        fg: Some(Color::YELLOW),
        ..Style::default()
    })?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "!")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].spans,
        [
            StyledSpan {
                style: Style {
                    dimmed: true,
                    fg: Some(Color::Index(2)),
                    bg: Some(Color::Index(7)),
                    ..Style::default()
                },
                text: "Hello, ".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(3)),
                    ..Style::default()
                },
                text: "world".into(),
            },
            "!".into(),
        ]
    );

    Ok(())
}

#[test]
fn html_writer_custom_colors() -> anyhow::Result<()> {
    let mut writer = LineWriter::new(None);
    writer.write_style(&Style {
        fg: Some(Color::Index(5)),
        ..Style::default()
    })?;
    write!(writer, "H")?;
    writer.write_style(&Style {
        bg: Some(Color::Index(14)),
        ..Style::default()
    })?;
    write!(writer, "e")?;
    writer.write_style(&Style {
        bg: Some(Color::Index(76)),
        ..Style::default()
    })?;
    write!(writer, "l")?;
    writer.write_style(&Style {
        fg: Some(Color::Index(200)),
        ..Style::default()
    })?;
    write!(writer, "l")?;
    writer.write_style(&Style {
        bg: Some(Color::Index(250)),
        ..Style::default()
    })?;
    write!(writer, "o")?;
    writer.reset()?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].spans,
        [
            StyledSpan {
                style: Style {
                    fg: Some(Color::Index(5)),
                    ..Style::default()
                },
                text: "H".into(),
            },
            StyledSpan {
                style: Style {
                    bg: Some(Color::Index(14)),
                    ..Style::default()
                },
                text: "e".into(),
            },
            StyledSpan {
                style: Style {
                    bg: Some(Color::Rgb("#5fd700".parse()?)),
                    ..Style::default()
                },
                text: "l".into(),
            },
            StyledSpan {
                style: Style {
                    fg: Some(Color::Rgb("#ff00d7".parse()?)),
                    ..Style::default()
                },
                text: "l".into(),
            },
            StyledSpan {
                style: Style {
                    bg: Some(Color::Rgb("#bcbcbc".parse()?)),
                    ..Style::default()
                },
                text: "o".into(),
            },
        ]
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
    let mut writer = LineWriter::new(Some(5));

    write!(writer, "Hello, ")?;
    writer.write_style(&Style {
        bold: true,
        underline: true,
        fg: Some(Color::GREEN),
        bg: Some(Color::WHITE),
        ..Style::default()
    })?;
    write!(writer, "world")?;
    writer.reset()?;
    write!(writer, "! More>\ntext")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].spans, ["Hello".into()]);
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(
        lines[1].spans,
        [
            ", ".into(),
            StyledSpan {
                style: Style {
                    bold: true,
                    underline: true,
                    fg: Some(Color::Index(2)),
                    bg: Some(Color::Index(7)),
                    ..Style::default()
                },
                text: "wor".into(),
            }
        ]
    );
    assert_eq!(lines[1].br, Some(LineBreak::Hard));
    assert_eq!(
        lines[2].spans,
        [
            StyledSpan {
                style: Style {
                    bold: true,
                    underline: true,
                    fg: Some(Color::Index(2)),
                    bg: Some(Color::Index(7)),
                    ..Style::default()
                },
                text: "ld".into(),
            },
            "! M".into(),
        ]
    );
    assert_eq!(lines[2].br, Some(LineBreak::Hard));
    assert_eq!(lines[3].spans, ["ore>".into()]);
    assert_eq!(lines[3].br, None);
    assert_eq!(lines[4].spans, ["text".into()]);
    assert_eq!(lines[4].br, None);

    Ok(())
}

#[test]
fn splitting_lines_with_escaped_chars() -> anyhow::Result<()> {
    let mut writer = LineWriter::new(Some(5));
    writeln!(writer, ">>>>>>>")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].spans, [">>>>>".into()]);
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(lines[1].spans, [">>".into()]);

    let mut writer = LineWriter::new(Some(5));
    for _ in 0..7 {
        write!(writer, ">")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].spans, [">>>>>".into()]);
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(lines[1].spans, [">>".into()]);
    Ok(())
}

#[test]
fn splitting_lines_with_newlines() -> anyhow::Result<()> {
    let mut writer = LineWriter::new(Some(5));

    for _ in 0..2 {
        writeln!(writer, "< test >")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].spans, ["< tes".into()]);
    assert_eq!(lines[1].spans, ["t >".into()]);
    assert_eq!(lines[2].spans, ["< tes".into()]);
    assert_eq!(lines[3].spans, ["t >".into()]);

    let mut writer = LineWriter::new(Some(5));
    for _ in 0..2 {
        writeln!(writer, "<< test >>")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].spans, ["<< te".into()]);
    assert_eq!(lines[1].spans, ["st >>".into()]);
    assert_eq!(lines[2].spans, ["<< te".into()]);
    assert_eq!(lines[3].spans, ["st >>".into()]);
    Ok(())
}
