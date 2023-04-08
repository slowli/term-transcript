use termcolor::{ColorSpec, WriteColor};

use std::{fmt, io, iter, mem, str};

use super::{IndexOrRgb, LineBreak, LineSplitter, StyledSpan, WriteLines, WriteStr};

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
        this.set_fg(color, intense, "fill");
        this
    }
}

#[derive(Debug)]
pub(crate) struct SvgLine {
    pub background: Option<String>,
    pub foreground: String,
}

impl SvgLine {
    fn new(foreground: String, background_segments: Vec<BackgroundSegment>) -> Self {
        let background = if let Some(segment) = background_segments.last() {
            let estimated_capacity =
                16 * background_segments.len() + segment.start_pos + segment.char_width;
            let mut background = String::with_capacity(estimated_capacity);
            let mut pos = 0;
            for segment in background_segments {
                background.extend(iter::repeat('\u{a0}').take(segment.start_pos - pos));
                pos = segment.start_pos + segment.char_width;
                segment.write_tspan(&mut background);
            }
            Some(background)
        } else {
            None
        };

        Self {
            background,
            foreground,
        }
    }
}

#[derive(Debug)]
struct BackgroundSegment {
    start_pos: usize,
    char_width: usize,
    span: StyledSpan,
}

impl BackgroundSegment {
    fn write_tspan(self, output: &mut String) {
        self.span.write_tag(output, "tspan").unwrap();
        output.extend(iter::repeat('█').take(self.char_width));
        output.push_str("</tspan>");
    }
}

#[derive(Debug)]
pub(crate) struct SvgWriter {
    output: Vec<SvgLine>,
    current_background: Vec<BackgroundSegment>,
    current_line: String,
    current_style: Option<ColorSpec>,
    line_splitter: LineSplitter,
}

impl SvgWriter {
    pub fn new(max_width: Option<usize>) -> Self {
        Self {
            output: vec![],
            current_background: vec![],
            current_line: String::new(),
            current_style: None,
            line_splitter: max_width.map_or_else(LineSplitter::default, LineSplitter::new),
        }
    }

    fn write_color(&mut self, spec: ColorSpec, start_pos: usize) -> io::Result<()> {
        use fmt::Write as _;

        let mut span = StyledSpan::new(&spec, "fill")?;

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
            self.current_background.push(BackgroundSegment {
                start_pos,
                char_width: 0,
                span: StyledSpan::for_bg(color, spec.intense(), spec.dimmed()),
            });
        }

        span.write_tag(self, "tspan")?;
        self.current_style = Some(spec);
        Ok(())
    }

    fn reset_inner(&mut self, line_width: Option<usize>) -> io::Result<()> {
        if let Some(spec) = &self.current_style {
            if spec.bg().is_some() {
                let line_width = line_width.unwrap_or(self.line_splitter.current_width);
                self.terminate_bg_segment(line_width);
            }
            self.current_style = None;
            self.write_str("</tspan>")?;
        }
        Ok(())
    }

    fn terminate_bg_segment(&mut self, current_width: usize) {
        let segment = self.current_background.last_mut().unwrap();
        segment.char_width = current_width - segment.start_pos;
    }

    pub fn close(mut self) -> Vec<SvgLine> {
        self.output.push(SvgLine::new(
            mem::take(&mut self.current_line),
            mem::take(&mut self.current_background),
        ));
        self.output
    }
}

impl WriteStr for SvgWriter {
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.current_line.write_str(s)
    }
}

impl WriteLines for SvgWriter {
    fn line_splitter_mut(&mut self) -> Option<&mut LineSplitter> {
        Some(&mut self.line_splitter)
    }

    fn write_line_break(&mut self, br: LineBreak, char_width: usize) -> io::Result<()> {
        const HARD_BR: &str =
            r#"<tspan class="hard-br" rotate="45" dx="-.1em" dy="-.25em">↓</tspan>"#;
        match br {
            LineBreak::Hard => self.write_str(HARD_BR)?,
        }
        self.write_new_line(char_width)
    }

    fn write_new_line(&mut self, char_width: usize) -> io::Result<()> {
        let current_style = self.current_style.clone();
        self.reset_inner(Some(char_width))?;

        self.output.push(SvgLine::new(
            mem::take(&mut self.current_line),
            mem::take(&mut self.current_background),
        ));

        if let Some(spec) = current_style {
            self.write_color(spec, 0)?;
        }
        Ok(())
    }
}

impl io::Write for SvgWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.io_write(buffer, true)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteColor for SvgWriter {
    fn supports_color(&self) -> bool {
        true
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        debug_assert!(spec.reset());
        self.reset()?;
        if !spec.is_none() {
            let start_pos = self.line_splitter.current_width;
            self.write_color(spec.clone(), start_pos)?;
        }
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.reset_inner(None)
    }
}

#[cfg(test)]
mod tests {
    use io::Write as _;
    use termcolor::Color;

    use super::*;

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

        let mut lines = writer.close();
        assert_eq!(lines.len(), 1);
        let SvgLine {
            background,
            foreground,
        } = lines.pop().unwrap();
        let background = background.unwrap();
        assert_eq!(
            background,
            "\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}<tspan class=\"fg7\">█████</tspan>"
        );
        assert_eq!(
            foreground,
            "Hello,\u{a0}<tspan class=\"bold underline fg2 bg7\">world</tspan>!"
        );
        Ok(())
    }

    #[test]
    fn svg_writer_intense_color() -> anyhow::Result<()> {
        let mut writer = SvgWriter::new(None);

        writer.set_color(ColorSpec::new().set_intense(true).set_fg(Some(Color::Blue)))?;
        write!(writer, "blue")?;
        writer.reset()?;

        let mut lines = writer.close();
        assert_eq!(lines.len(), 1);
        let SvgLine {
            background,
            foreground,
        } = lines.pop().unwrap();
        assert!(background.is_none());
        assert_eq!(foreground, r#"<tspan class="fg12">blue</tspan>"#);
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

        let mut lines = writer.close();
        assert_eq!(lines.len(), 1);
        let SvgLine {
            background,
            foreground,
        } = lines.pop().unwrap();
        let background = background.unwrap();
        assert_eq!(
            background,
            "\u{a0}<tspan class=\"fg14\">█</tspan>\
             <tspan style=\"fill: #5fd700;\">█</tspan>\
             \u{a0}<tspan style=\"fill: #bcbcbc;\">█</tspan>"
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

        let lines = writer.close();
        let [first, second, third] = lines.as_slice() else {
            panic!("Unexpected lines: {lines:?}");
        };
        assert!(first.background.is_none());
        assert_eq!(first.foreground, "Hello,");
        assert_eq!(
            second.background.as_ref().unwrap(),
            "\u{a0}<tspan class=\"fg7\">███</tspan>"
        );
        assert_eq!(
            second.foreground,
            "\u{a0}<tspan class=\"bold underline fg2 bg7\">wor</tspan>"
        );
        assert_eq!(
            third.background.as_ref().unwrap(),
            "<tspan class=\"fg7\">██</tspan>"
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

        let lines = writer.close();
        assert_eq!(lines.len(), 5);
        let [first, second, third, ..] = lines.as_slice() else { unreachable!() };
        assert!(first.background.is_none());
        assert_eq!(
            first.foreground,
            "Hello<tspan class=\"hard-br\" rotate=\"45\" dx=\"-.1em\" dy=\"-.25em\">↓</tspan>"
        );
        assert_eq!(
            second.background.as_ref().unwrap(),
            "\u{a0}\u{a0}<tspan class=\"fg7\">███</tspan>"
        );
        assert_eq!(
            second.foreground,
            ",\u{a0}<tspan class=\"bold underline fg2 bg7\">wor\
             <tspan class=\"hard-br\" rotate=\"45\" dx=\"-.1em\" dy=\"-.25em\">↓</tspan></tspan>"
        );
        assert_eq!(
            third.background.as_ref().unwrap(),
            "<tspan class=\"fg7\">██</tspan>"
        );
        assert_eq!(
            third.foreground,
            "<tspan class=\"bold underline fg2 bg7\">ld</tspan>!\u{a0}M\
             <tspan class=\"hard-br\" rotate=\"45\" dx=\"-.1em\" dy=\"-.25em\">↓</tspan>"
        );

        assert!(lines[3].background.is_none());
        assert_eq!(lines[3].foreground, "ore&gt;");
        assert!(lines[4].background.is_none());
        assert_eq!(lines[4].foreground, "text");

        Ok(())
    }
}
