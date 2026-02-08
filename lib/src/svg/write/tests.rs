use super::*;

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

    writer.write_text("Hello, ")?;
    writer.write_style(&Style {
        bold: true,
        underline: true,
        fg: Some(Color::GREEN),
        bg: Some(Color::WHITE),
        ..Style::default()
    })?;
    writer.write_text("world")?;
    writer.reset()?;
    writer.write_text("! More>\ntext")?;

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
    writer.write_text(">>>>>>>\n")?;

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].spans, [">>>>>".into()]);
    assert_eq!(lines[0].br, Some(LineBreak::Hard));
    assert_eq!(lines[1].spans, [">>".into()]);

    let mut writer = LineWriter::new(Some(5));
    for _ in 0..7 {
        writer.write_text(">")?;
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
        writer.write_text("< test >\n")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].spans, ["< tes".into()]);
    assert_eq!(lines[1].spans, ["t >".into()]);
    assert_eq!(lines[2].spans, ["< tes".into()]);
    assert_eq!(lines[3].spans, ["t >".into()]);

    let mut writer = LineWriter::new(Some(5));
    for _ in 0..2 {
        writer.write_text("<< test >>\n")?;
    }

    let lines = writer.into_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].spans, ["<< te".into()]);
    assert_eq!(lines[1].spans, ["st >>".into()]);
    assert_eq!(lines[2].spans, ["<< te".into()]);
    assert_eq!(lines[3].spans, ["st >>".into()]);
    Ok(())
}
