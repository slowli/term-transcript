use termcolor::{Color, ColorSpec, WriteColor};
use unicode_width::UnicodeWidthChar;

use std::{fmt::Write as WriteStr, io, str};

/// `WriteColor` implementation that renders output as HTML.
///
/// **NB.** The implementation relies on `ColorSpec`s supplied to `set_color` always having
/// `reset()` flag set. This is true for `TermOutputParser`.
pub struct HtmlWriter<'a> {
    output: &'a mut dyn WriteStr,
    is_colored: bool,
    line_splitter: Option<LineSplitter>,
}

impl<'a> HtmlWriter<'a> {
    pub fn new(output: &'a mut dyn WriteStr, max_width: Option<usize>) -> Self {
        Self {
            output,
            is_colored: false,
            line_splitter: max_width.map(LineSplitter::new),
        }
    }

    /// Writes the specified string as-is tp the underlying `output`.
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.output
            .write_str(s)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }

    /// Writes the specified text displayed to the user that should be subjected to wrapping.
    #[allow(clippy::option_if_let_else)] // false positive
    fn write_text(&mut self, s: &str) -> io::Result<()> {
        if let Some(splitter) = &mut self.line_splitter {
            let lines = splitter.split_lines(s);
            self.write_lines(lines)
        } else {
            self.write_str(s)
        }
    }

    fn write_lines(&mut self, lines: Vec<Line<'_>>) -> io::Result<()> {
        let lines_count = lines.len();
        for (i, Line { text, br }) in lines.into_iter().enumerate() {
            self.write_str(text)?;
            if let Some(br) = br {
                self.write_str(br.as_html())?;
            } else if i + 1 < lines_count {
                self.write_str("\n")?;
            }
        }
        Ok(())
    }

    /// Writes the specified HTML `entity` as if it were displayed as a single char.
    #[allow(clippy::option_if_let_else)] // false positive
    fn write_html_entity(&mut self, entity: &str) -> io::Result<()> {
        if let Some(splitter) = &mut self.line_splitter {
            let lines = splitter.write_as_char(entity);
            self.write_lines(lines)
        } else {
            self.write_str(entity)
        }
    }

    fn write_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        let mut classes = vec![];
        if spec.bold() {
            classes.push("bold");
        }
        if spec.dimmed() {
            classes.push("dimmed");
        }
        if spec.italic() {
            classes.push("italic");
        }
        if spec.underline() {
            classes.push("underline");
        }

        let mut styles = vec![];

        let mut fore_color_class = String::with_capacity(4);
        fore_color_class.push_str("fg");
        let fore_color = spec.fg().map(|&color| IndexOrRgb::new(color)).transpose()?;
        match fore_color {
            Some(IndexOrRgb::Index(idx)) => {
                let final_idx = if spec.intense() { idx | 8 } else { idx };
                write!(&mut fore_color_class, "{}", final_idx).unwrap();
                // ^-- `unwrap` is safe; writing to a string never fails.
                classes.push(&fore_color_class);
            }
            Some(IndexOrRgb::Rgb(r, g, b)) => {
                styles.push(format!("color: #{:02x}{:02x}{:02x}", r, g, b));
            }
            None => { /* Do nothing. */ }
        }

        let mut back_color_class = String::with_capacity(4);
        back_color_class.push_str("bg");
        let back_color = spec.bg().map(|&color| IndexOrRgb::new(color)).transpose()?;
        match back_color {
            Some(IndexOrRgb::Index(idx)) => {
                let final_idx = if spec.intense() { idx | 8 } else { idx };
                write!(&mut back_color_class, "{}", final_idx).unwrap();
                // ^-- `unwrap` is safe; writing to a string never fails.
                classes.push(&back_color_class);
            }
            Some(IndexOrRgb::Rgb(r, g, b)) => {
                styles.push(format!("background: #{:02x}{:02x}{:02x}", r, g, b));
            }
            None => { /* Do nothing. */ }
        }

        self.write_str("<span")?;
        if !classes.is_empty() {
            self.write_str(" class=\"")?;
            for (i, &class) in classes.iter().enumerate() {
                self.write_str(class)?;
                if i + 1 < classes.len() {
                    self.write_str(" ")?;
                }
            }
            self.write_str("\"")?;
        }
        if !styles.is_empty() {
            self.write_str(" style=\"")?;
            for (i, style) in styles.iter().enumerate() {
                self.write_str(style)?;
                if i + 1 < styles.len() {
                    self.write_str("; ")?;
                }
            }
            self.write_str(";\"")?;
        }
        self.write_str(">")?;

        Ok(())
    }
}

impl io::Write for HtmlWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut last_escape = 0;
        for (i, &byte) in buffer.iter().enumerate() {
            let escaped = match byte {
                b'>' => "&gt;",
                b'<' => "&lt;",
                b'&' => "&amp;",
                _ => continue,
            };
            let saved_str = str::from_utf8(&buffer[last_escape..i])
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            self.write_text(saved_str)?;
            self.write_html_entity(escaped)?;
            last_escape = i + 1;
        }

        let saved_str = str::from_utf8(&buffer[last_escape..])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        self.write_text(saved_str)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteColor for HtmlWriter<'_> {
    fn supports_color(&self) -> bool {
        true
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        debug_assert!(spec.reset());
        self.reset()?;
        if !spec.is_none() {
            self.write_color(spec)?;
            self.is_colored = true;
        }
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        if self.is_colored {
            self.is_colored = false;
            self.write_str("</span>")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum IndexOrRgb {
    Index(u8),
    Rgb(u8, u8, u8),
}

impl IndexOrRgb {
    #[allow(clippy::match_wildcard_for_single_variants)]
    // ^-- `Color` is an old-school non-exhaustive enum
    fn new(color: Color) -> io::Result<Self> {
        Ok(match color {
            Color::Black => Self::index(0),
            Color::Red => Self::index(1),
            Color::Green => Self::index(2),
            Color::Yellow => Self::index(3),
            Color::Blue => Self::index(4),
            Color::Magenta => Self::index(5),
            Color::Cyan => Self::index(6),
            Color::White => Self::index(7),
            Color::Ansi256(idx) => Self::indexed_color(idx),
            Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
            _ => return Err(io::Error::new(io::ErrorKind::Other, "Unsupported color")),
        })
    }

    fn index(value: u8) -> Self {
        debug_assert!(value < 16);
        Self::Index(value)
    }

    fn indexed_color(index: u8) -> Self {
        match index {
            0..=15 => Self::index(index),

            16..=231 => {
                let index = index - 16;
                let r = Self::color_cube_color(index / 36);
                let g = Self::color_cube_color((index / 6) % 6);
                let b = Self::color_cube_color(index % 6);
                Self::Rgb(r, g, b)
            }

            _ => {
                let gray = 10 * (index - 232) + 8;
                Self::Rgb(gray, gray, gray)
            }
        }
    }

    fn color_cube_color(index: u8) -> u8 {
        match index {
            0 => 0,
            1 => 0x5f,
            2 => 0x87,
            3 => 0xaf,
            4 => 0xd7,
            5 => 0xff,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
struct LineSplitter {
    max_width: usize,
    current_width: usize,
}

impl LineSplitter {
    fn new(max_width: usize) -> Self {
        Self {
            max_width,
            current_width: 0,
        }
    }

    fn split_lines<'a>(&mut self, text: &'a str) -> Vec<Line<'a>> {
        text.lines()
            .chain(if text.ends_with('\n') { Some("") } else { None })
            .enumerate()
            .flat_map(|(i, line)| {
                if i > 0 {
                    self.current_width = 0;
                }
                self.process_line(line)
            })
            .collect()
    }

    fn write_as_char<'a>(&mut self, text: &'a str) -> Vec<Line<'a>> {
        if self.current_width + 1 > self.max_width {
            self.current_width = 1;
            vec![
                Line {
                    text: "",
                    br: Some(LineBreak::Hard),
                },
                Line { text, br: None },
            ]
        } else {
            self.current_width += 1;
            vec![Line { text, br: None }]
        }
    }

    fn process_line<'a>(&mut self, line: &'a str) -> Vec<Line<'a>> {
        let mut output_lines = vec![];
        let mut line_start = 0;

        for (pos, char) in line.char_indices() {
            let char_width = char.width().unwrap_or(0);
            if self.current_width + char_width > self.max_width {
                output_lines.push(Line {
                    text: &line[line_start..pos],
                    br: Some(LineBreak::Hard),
                });
                line_start = pos;
                self.current_width = char_width;
            } else {
                self.current_width += char_width;
            }
        }

        output_lines.push(Line {
            text: &line[line_start..],
            br: None,
        });
        output_lines
    }
}

#[derive(Debug, PartialEq)]
struct Line<'a> {
    text: &'a str,
    br: Option<LineBreak>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineBreak {
    Hard,
}

impl LineBreak {
    fn as_html(self) -> &'static str {
        match self {
            Self::Hard => r#"<b class="hard-br"><br/></b>"#,
        }
    }
}

#[cfg(test)]
mod tests {
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
        let lines = splitter
            .split_lines("tex text \u{7d75}\u{6587}\u{5b57}\n\u{1f602}\u{1f602}\u{1f602}\n");

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
}
