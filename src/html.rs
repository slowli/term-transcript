use termcolor::{Color, ColorSpec, WriteColor};

use std::{fmt::Write as WriteStr, io, str};

pub struct HtmlWriter<'a> {
    output: &'a mut dyn WriteStr,
    opened_spans: usize,
    current_spec: Option<ColorSpec>,
}

impl<'a> HtmlWriter<'a> {
    pub fn new(output: &'a mut dyn WriteStr) -> Self {
        Self {
            output,
            opened_spans: 0,
            current_spec: None,
        }
    }

    fn push_spec(&mut self, spec: &ColorSpec) {
        let current_spec = self.current_spec.get_or_insert_with(ColorSpec::new);
        if spec.bold() {
            current_spec.set_bold(true);
        }
        if spec.dimmed() {
            current_spec.set_dimmed(true);
        }
        if spec.italic() {
            current_spec.set_italic(true);
        }
        if spec.underline() {
            current_spec.set_underline(true);
        }
        if let Some(color) = spec.fg() {
            current_spec.set_fg(Some(*color));
        }
        if let Some(color) = spec.bg() {
            current_spec.set_bg(Some(*color));
        }
    }

    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.output
            .write_str(s)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
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
        self.opened_spans += 1;

        Ok(())
    }
}

impl io::Write for HtmlWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(spec) = self.current_spec.take() {
            self.write_color(&spec)?;
        }

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
            self.write_str(saved_str)?;
            self.write_str(escaped)?;
            last_escape = i + 1;
        }

        let saved_str = str::from_utf8(&buffer[last_escape..])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        self.write_str(saved_str)?;
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
        if spec.reset() {
            self.reset()?;
        }
        if !spec.is_none() {
            self.push_spec(spec);
        }
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        for _ in 0..self.opened_spans {
            self.write_str("</span>")?;
        }
        self.opened_spans = 0;
        self.current_spec = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    #[test]
    fn html_escaping() -> anyhow::Result<()> {
        let mut buffer = String::new();
        let mut writer = HtmlWriter::new(&mut buffer);
        write!(writer, "1 < 2 && 4 >= 3")?;

        assert_eq!(buffer, "1 &lt; 2 &amp;&amp; 4 &gt;= 3");
        Ok(())
    }

    #[test]
    fn html_writer_basic_colors() -> anyhow::Result<()> {
        let mut buffer = String::new();
        let mut writer = HtmlWriter::new(&mut buffer);
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
    fn html_writer_embedded_spans_with_reset() -> anyhow::Result<()> {
        let mut buffer = String::new();
        let mut writer = HtmlWriter::new(&mut buffer);
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
    fn html_writer_embedded_spans_without_reset() -> anyhow::Result<()> {
        let mut buffer = String::new();
        let mut writer = HtmlWriter::new(&mut buffer);
        writer.set_color(
            ColorSpec::new()
                .set_dimmed(true)
                .set_fg(Some(Color::Green))
                .set_bg(Some(Color::White)),
        )?;
        write!(writer, "Hello, ")?;
        writer.set_color(
            ColorSpec::new()
                .set_reset(false)
                .set_fg(Some(Color::Yellow)),
        )?;
        write!(writer, "world")?;
        writer.reset()?;
        write!(writer, "!")?;

        assert_eq!(
            buffer,
            "<span class=\"dimmed fg2 bg7\">Hello, <span class=\"fg3\">world</span></span>!"
        );

        Ok(())
    }

    #[test]
    fn html_writer_custom_colors() -> anyhow::Result<()> {
        let mut buffer = String::new();
        let mut writer = HtmlWriter::new(&mut buffer);
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
}
