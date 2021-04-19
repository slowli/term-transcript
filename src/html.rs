use handlebars::Output;
use termcolor::{Color, ColorSpec, WriteColor};

use std::{
    io::{self, Write},
    str,
};

pub struct HtmlWriter<'a> {
    output: &'a mut dyn Output,
    opened_spans: usize,
    current_spec: Option<ColorSpec>,
}

impl<'a> HtmlWriter<'a> {
    pub fn new(output: &'a mut dyn Output) -> Self {
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

        let fore_color = spec.fg().map(|&color| ClassOrRgb::new(color)).transpose()?;
        match fore_color {
            Some(ClassOrRgb::Class { value, intense }) => {
                let intense = intense || spec.intense();
                classes.push(value.as_str(false, intense));
            }
            Some(ClassOrRgb::Rgb(r, g, b)) => {
                styles.push(format!("color: #{:02x}{:02x}{:02x}", r, g, b));
            }
            None => { /* Do nothing. */ }
        }

        let back_color = spec.bg().map(|&color| ClassOrRgb::new(color)).transpose()?;
        match back_color {
            Some(ClassOrRgb::Class { value, intense }) => {
                let intense = intense || spec.intense();
                classes.push(value.as_str(true, intense));
            }
            Some(ClassOrRgb::Rgb(r, g, b)) => {
                styles.push(format!("background-color: #{:02x}{:02x}{:02x}", r, g, b));
            }
            None => { /* Do nothing. */ }
        }

        self.output.write("<span")?;
        if !classes.is_empty() {
            self.output.write(" class=\"")?;
            for (i, &class) in classes.iter().enumerate() {
                self.output.write(class)?;
                if i + 1 < classes.len() {
                    self.output.write(" ")?;
                }
            }
            self.output.write("\"")?;
        }
        if !styles.is_empty() {
            self.output.write(" style=\"")?;
            for (i, style) in styles.iter().enumerate() {
                self.output.write(style)?;
                if i + 1 < styles.len() {
                    self.output.write("; ")?;
                }
            }
            self.output.write("\"")?;
        }
        self.output.write(">")?;
        self.opened_spans += 1;

        Ok(())
    }
}

impl Write for HtmlWriter<'_> {
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
            self.output.write(saved_str)?;
            self.output.write(escaped)?;
            last_escape = i + 1;
        }

        let saved_str = str::from_utf8(&buffer[last_escape..])
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        self.output.write(saved_str)?;
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
            self.output.write("</span>")?;
        }
        self.opened_spans = 0;
        self.current_spec = None;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum ColorClass {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl ColorClass {
    fn as_str(self, background: bool, intense: bool) -> &'static str {
        macro_rules! get_str {
            ($prefix:tt) => {
                match self {
                    Self::Black => concat!($prefix, "-black"),
                    Self::Red => concat!($prefix, "-red"),
                    Self::Green => concat!($prefix, "-green"),
                    Self::Yellow => concat!($prefix, "-yellow"),
                    Self::Blue => concat!($prefix, "-blue"),
                    Self::Magenta => concat!($prefix, "-magenta"),
                    Self::Cyan => concat!($prefix, "-cyan"),
                    Self::White => concat!($prefix, "-white"),
                }
            };
        }

        match (background, intense) {
            (false, false) => get_str!("fg"),
            (false, true) => get_str!("fg-i"),
            (true, false) => get_str!("bg"),
            (true, true) => get_str!("bg-i"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ClassOrRgb {
    Class { value: ColorClass, intense: bool },
    Rgb(u8, u8, u8),
}

impl ClassOrRgb {
    #[allow(clippy::match_wildcard_for_single_variants)]
    // ^-- `Color` is an old-school non-exhaustive enum
    fn new(color: Color) -> io::Result<Self> {
        Ok(match color {
            Color::Black => Self::class(ColorClass::Black),
            Color::Red => Self::class(ColorClass::Red),
            Color::Green => Self::class(ColorClass::Green),
            Color::Yellow => Self::class(ColorClass::Yellow),
            Color::Blue => Self::class(ColorClass::Blue),
            Color::Magenta => Self::class(ColorClass::Magenta),
            Color::Cyan => Self::class(ColorClass::Cyan),
            Color::White => Self::class(ColorClass::White),
            Color::Ansi256(idx) => Self::indexed_color(idx),
            Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
            _ => return Err(io::Error::new(io::ErrorKind::Other, "Unsupported color")),
        })
    }

    fn class(value: ColorClass) -> Self {
        Self::Class {
            value,
            intense: false,
        }
    }

    fn intense_class(value: ColorClass) -> Self {
        Self::Class {
            value,
            intense: true,
        }
    }

    fn indexed_color(index: u8) -> Self {
        match index {
            0 => Self::class(ColorClass::Black),
            1 => Self::class(ColorClass::Red),
            2 => Self::class(ColorClass::Green),
            3 => Self::class(ColorClass::Yellow),
            4 => Self::class(ColorClass::Blue),
            5 => Self::class(ColorClass::Magenta),
            6 => Self::class(ColorClass::Cyan),
            7 => Self::class(ColorClass::White),

            8 => Self::intense_class(ColorClass::Black),
            9 => Self::intense_class(ColorClass::Red),
            10 => Self::intense_class(ColorClass::Green),
            11 => Self::intense_class(ColorClass::Yellow),
            12 => Self::intense_class(ColorClass::Blue),
            13 => Self::intense_class(ColorClass::Magenta),
            14 => Self::intense_class(ColorClass::Cyan),
            15 => Self::intense_class(ColorClass::White),

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

    use std::io::Error;
    use std::str;

    #[derive(Debug, Default)]
    struct StringOutput(String);

    impl Output for StringOutput {
        fn write(&mut self, seg: &str) -> Result<(), Error> {
            self.0.push_str(seg);
            Ok(())
        }
    }

    #[test]
    fn html_escaping() -> anyhow::Result<()> {
        let mut buffer = StringOutput::default();
        let mut writer = HtmlWriter::new(&mut buffer);
        write!(writer, "1 < 2 && 4 >= 3")?;

        assert_eq!(buffer.0, "1 &lt; 2 &amp;&amp; 4 &gt;= 3");
        Ok(())
    }

    #[test]
    fn html_writer_basic_colors() -> anyhow::Result<()> {
        let mut buffer = StringOutput::default();
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
            buffer.0,
            r#"Hello, <span class="bold underline fg-green bg-white">world</span>!"#
        );

        Ok(())
    }

    #[test]
    fn html_writer_embedded_spans_with_reset() -> anyhow::Result<()> {
        let mut buffer = StringOutput::default();
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
            buffer.0,
            "<span class=\"dimmed fg-green bg-white\">Hello, </span>\
             <span class=\"fg-yellow\">world</span>!"
        );

        Ok(())
    }

    #[test]
    fn html_writer_embedded_spans_without_reset() -> anyhow::Result<()> {
        let mut buffer = StringOutput::default();
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
            buffer.0,
            "<span class=\"dimmed fg-green bg-white\">Hello, \
             <span class=\"fg-yellow\">world</span></span>!"
        );

        Ok(())
    }

    #[test]
    fn html_writer_custom_colors() -> anyhow::Result<()> {
        let mut buffer = StringOutput::default();
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
            buffer.0,
            "<span class=\"fg-magenta\">H</span>\
             <span class=\"bg-i-cyan\">e</span>\
             <span style=\"background-color: #5fd700\">l</span>\
             <span style=\"color: #ff00d7\">l</span>\
             <span style=\"background-color: #bcbcbc\">o</span>"
        );
        Ok(())
    }
}
