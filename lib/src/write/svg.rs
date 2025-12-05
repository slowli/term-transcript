use std::{fmt, io};

use serde::Serialize;
use termcolor::ColorSpec;

use super::{IndexOrRgb, LineBreak, StyledLine, StyledSpan};

impl StyledSpan {
    fn for_bg(color: IndexOrRgb, intense: bool, dimmed: bool) -> Self {
        let classes = if dimmed {
            vec!["dimmed".to_owned()]
        } else {
            vec![]
        };
        let mut this = Self {
            classes,
            styles: vec![],
        };
        this.set_fg(color, intense, &["fill", "stroke"]);
        // ^ Ideally, we'd want to add `stroke: context-fill` to the `.output-bg` selector.
        // Unfortunately, it's not supported by all viewers.
        this
    }
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct SvgLine {
    pub background: Vec<BackgroundSegment>,
    pub foreground: String,
    pub br: Option<LineBreak>,
}

#[derive(Debug, PartialEq, Serialize)]
pub(crate) struct BackgroundSegment {
    start_pos: usize,
    char_width: usize,
    attrs: String,
}

impl AsMut<String> for SvgLine {
    fn as_mut(&mut self) -> &mut String {
        &mut self.foreground
    }
}

impl StyledLine for SvgLine {
    fn write_color(&mut self, spec: &ColorSpec, start_pos: usize) -> io::Result<()> {
        use fmt::Write as _;

        let mut span = StyledSpan::new(spec, "fill")?;

        let mut back_color_class = String::with_capacity(4);
        back_color_class.push_str("bg");
        let back_color = spec.bg().map(|&color| IndexOrRgb::new(color)).transpose()?;
        match back_color {
            Some(IndexOrRgb::Index(idx)) => {
                let final_idx = if spec.intense() { idx | 8 } else { idx };
                write!(&mut back_color_class, "{final_idx}").unwrap();
                // ^-- `unwrap` is safe; writing to a string never fails.
                span.classes.push(back_color_class);
            }
            Some(IndexOrRgb::Rgb(r, g, b)) => {
                write!(&mut back_color_class, "#{r:02x}{g:02x}{b:02x}").unwrap();
                span.classes.push(back_color_class);
            }
            None => { /* Do nothing. */ }
        }
        if let Some(color) = back_color {
            let span = StyledSpan::for_bg(color, spec.intense(), spec.dimmed());
            let mut attrs = String::new();
            span.write_attrs(&mut attrs);
            self.background.push(BackgroundSegment {
                start_pos,
                char_width: 0,
                attrs,
            });
        }

        span.write_tag(&mut self.foreground, "tspan");
        Ok(())
    }

    fn reset_color(&mut self, prev_spec: &ColorSpec, current_width: usize) {
        if prev_spec.bg().is_some() {
            let segment = self.background.last_mut().unwrap();
            segment.char_width = current_width - segment.start_pos;
        }
        self.foreground.push_str("</tspan>");
    }

    fn set_br(&mut self, br: Option<LineBreak>) {
        self.br = br;
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use termcolor::{Color, WriteColor as _};

    use super::*;
    use crate::write::GenericWriter;

    type SvgWriter = GenericWriter<SvgLine>;

    #[test]
    fn svg_writer_basic_colors() -> anyhow::Result<()> {
        let mut writer = SvgWriter::new(None);
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

        let mut lines = writer.into_lines();
        assert_eq!(lines.len(), 1);
        let SvgLine {
            background,
            foreground,
            ..
        } = lines.pop().unwrap();
        assert_eq!(
            background,
            [BackgroundSegment {
                start_pos: 7,
                char_width: 5,
                attrs: " class=\"fg7\"".to_owned(),
            }]
        );
        assert_eq!(
            foreground,
            "Hello, <tspan class=\"bold underline fg2 bg7\">world</tspan>!"
        );
        Ok(())
    }

    #[test]
    fn svg_writer_intense_color() -> anyhow::Result<()> {
        let mut writer = SvgWriter::new(None);

        writer.set_color(ColorSpec::new().set_intense(true).set_fg(Some(Color::Blue)))?;
        write!(writer, "blue")?;
        writer.reset()?;

        let mut lines = writer.into_lines();
        assert_eq!(lines.len(), 1);
        let SvgLine {
            background,
            foreground,
            ..
        } = lines.pop().unwrap();
        assert!(background.is_empty());
        assert_eq!(foreground, r#"<tspan class="fg12">blue</tspan>"#);
        Ok(())
    }

    #[test]
    fn final_empty_line_in_writer() -> anyhow::Result<()> {
        let writer = SvgWriter::new(None);
        let lines = writer.into_lines();
        assert!(lines.is_empty());

        let mut writer = SvgWriter::new(None);

        writer.set_color(ColorSpec::new().set_intense(true).set_fg(Some(Color::Blue)))?;
        write!(writer, "")?;
        writer.reset()?;

        let lines = writer.into_lines();
        assert!(lines.is_empty());
        Ok(())
    }

    #[test]
    fn svg_writer_custom_colors() -> anyhow::Result<()> {
        let mut writer = SvgWriter::new(None);
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

        let mut lines = writer.into_lines();
        assert_eq!(lines.len(), 1);
        let SvgLine {
            background,
            foreground,
            ..
        } = lines.pop().unwrap();
        assert_eq!(
            background,
            [
                BackgroundSegment {
                    start_pos: 1,
                    char_width: 1,
                    attrs: " class=\"fg14\"".to_owned(),
                },
                BackgroundSegment {
                    start_pos: 2,
                    char_width: 1,
                    attrs: " style=\"fill: #5fd700; stroke: #5fd700;\"".to_owned(),
                },
                BackgroundSegment {
                    start_pos: 4,
                    char_width: 1,
                    attrs: " style=\"fill: #bcbcbc; stroke: #bcbcbc;\"".to_owned()
                },
            ]
        );

        assert_eq!(
            foreground,
            "<tspan class=\"fg5\">H</tspan>\
             <tspan class=\"bg14\">e</tspan>\
             <tspan class=\"bg#5fd700\">l</tspan>\
             <tspan style=\"fill: #ff00d7;\">l</tspan>\
             <tspan class=\"bg#bcbcbc\">o</tspan>"
        );

        Ok(())
    }

    #[test]
    fn svg_writer_newlines() -> anyhow::Result<()> {
        let mut writer = SvgWriter::new(None);
        writeln!(writer, "Hello,")?;
        write!(writer, " ")?;
        writer.set_color(
            ColorSpec::new()
                .set_bold(true)
                .set_underline(true)
                .set_fg(Some(Color::Green))
                .set_bg(Some(Color::White)),
        )?;
        writeln!(writer, "wor")?;
        write!(writer, "ld")?;
        writer.reset()?;
        write!(writer, "!")?;

        let lines = writer.into_lines();
        let [first, second, third] = lines.as_slice() else {
            panic!("Unexpected lines: {lines:?}");
        };
        assert!(first.background.is_empty());
        assert_eq!(first.foreground, "Hello,");
        assert_eq!(
            second.background,
            [BackgroundSegment {
                start_pos: 1,
                char_width: 3,
                attrs: " class=\"fg7\"".to_owned(),
            }]
        );
        assert_eq!(
            second.foreground,
            " <tspan class=\"bold underline fg2 bg7\">wor</tspan>"
        );
        assert_eq!(
            third.background,
            [BackgroundSegment {
                start_pos: 0,
                char_width: 2,
                attrs: " class=\"fg7\"".to_owned(),
            }]
        );
        assert_eq!(
            third.foreground,
            "<tspan class=\"bold underline fg2 bg7\">ld</tspan>!"
        );

        Ok(())
    }

    #[test]
    fn splitting_lines_in_svg_writer() -> anyhow::Result<()> {
        let mut writer = SvgWriter::new(Some(5));

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
        let [first, second, third, ..] = lines.as_slice() else {
            unreachable!();
        };
        assert!(first.background.is_empty());
        assert_eq!(first.foreground, "Hello");
        assert_eq!(first.br, Some(LineBreak::Hard));
        assert_eq!(
            second.background,
            [BackgroundSegment {
                start_pos: 2,
                char_width: 3,
                attrs: " class=\"fg7\"".to_owned(),
            }]
        );
        assert_eq!(
            second.foreground,
            ", <tspan class=\"bold underline fg2 bg7\">wor</tspan>"
        );
        assert_eq!(second.br, Some(LineBreak::Hard));
        assert_eq!(
            third.background,
            [BackgroundSegment {
                start_pos: 0,
                char_width: 2,
                attrs: " class=\"fg7\"".to_owned(),
            }]
        );
        assert_eq!(
            third.foreground,
            "<tspan class=\"bold underline fg2 bg7\">ld</tspan>! M"
        );
        assert_eq!(third.br, Some(LineBreak::Hard));

        assert!(lines[3].background.is_empty());
        assert_eq!(lines[3].foreground, "ore&gt;");
        assert!(lines[4].background.is_empty());
        assert_eq!(lines[4].foreground, "text");

        Ok(())
    }
}
