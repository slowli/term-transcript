//! Rendering logic for terminal outputs.

use termcolor::{Color, ColorSpec};
use unicode_width::UnicodeWidthChar;

use std::{fmt, io, str};

mod html;
mod svg;
#[cfg(test)]
mod tests;

pub(crate) use self::{
    html::HtmlWriter,
    svg::{SvgLine, SvgWriter},
};

fn fmt_to_io_error(err: fmt::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

/// HTML `<span>` / SVG `<tspan>` containing styling info.
#[derive(Debug)]
struct StyledSpan {
    classes: Vec<String>,
    styles: Vec<String>,
}

impl StyledSpan {
    fn new(spec: &ColorSpec, fg_property: &str) -> io::Result<Self> {
        let mut classes = vec![];
        if spec.bold() {
            classes.push("bold".to_owned());
        }
        if spec.dimmed() {
            classes.push("dimmed".to_owned());
        }
        if spec.italic() {
            classes.push("italic".to_owned());
        }
        if spec.underline() {
            classes.push("underline".to_owned());
        }

        let mut this = Self {
            classes,
            styles: vec![],
        };
        if let Some(color) = spec.fg() {
            let color = IndexOrRgb::new(*color)?;
            this.set_fg(color, spec.intense(), &[fg_property]);
        }
        Ok(this)
    }

    fn set_fg(&mut self, color: IndexOrRgb, intense: bool, fg_properties: &[&str]) {
        use fmt::Write as _;

        let mut fore_color_class = String::with_capacity(4);
        fore_color_class.push_str("fg");
        match color {
            IndexOrRgb::Index(idx) => {
                let final_idx = if intense { idx | 8 } else { idx };
                write!(&mut fore_color_class, "{final_idx}").unwrap();
                // ^-- `unwrap` is safe; writing to a string never fails.
                self.classes.push(fore_color_class);
            }
            IndexOrRgb::Rgb(r, g, b) => {
                for property in fg_properties {
                    self.styles
                        .push(format!("{property}: #{r:02x}{g:02x}{b:02x}"));
                }
            }
        }
    }

    fn write_tag(self, output: &mut impl WriteStr, tag: &str) -> io::Result<()> {
        output.write_str("<")?;
        output.write_str(tag)?;
        if !self.classes.is_empty() {
            output.write_str(" class=\"")?;
            for (i, class) in self.classes.iter().enumerate() {
                output.write_str(class)?;
                if i + 1 < self.classes.len() {
                    output.write_str(" ")?;
                }
            }
            output.write_str("\"")?;
        }
        if !self.styles.is_empty() {
            output.write_str(" style=\"")?;
            for (i, style) in self.styles.iter().enumerate() {
                output.write_str(style)?;
                if i + 1 < self.styles.len() {
                    output.write_str("; ")?;
                }
            }
            output.write_str(";\"")?;
        }
        output.write_str(">")
    }
}

/// Analogue of `std::fmt::Write`, but with `io::Error`s.
trait WriteStr {
    fn write_str(&mut self, s: &str) -> io::Result<()>;
}

impl WriteStr for String {
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        <Self as fmt::Write>::write_str(self, s).map_err(fmt_to_io_error)
    }
}

/// Shared logic between `HtmlWriter` and `SvgWriter`.
trait WriteLines: WriteStr {
    fn line_splitter_mut(&mut self) -> Option<&mut LineSplitter>;

    /// Writes a [`LineBreak`] to this writer. The char width of the line preceding the break
    /// is `char_width`.
    fn write_line_break(&mut self, br: LineBreak, char_width: usize) -> io::Result<()>;

    /// Writes a newline `\n` to this writer.
    fn write_new_line(&mut self, char_width: usize) -> io::Result<()>;

    /// Writes the specified text displayed to the user that should be subjected to wrapping.
    #[allow(clippy::option_if_let_else)] // false positive
    fn write_text(&mut self, s: &str) -> io::Result<()> {
        if let Some(splitter) = self.line_splitter_mut() {
            let lines = splitter.split_lines(s);
            self.write_lines(lines)
        } else {
            self.write_str(s)
        }
    }

    fn write_lines(&mut self, lines: Vec<Line<'_>>) -> io::Result<()> {
        let lines_count = lines.len();
        let it = lines.into_iter().enumerate();
        for (i, line) in it {
            self.write_str(line.text)?;
            if let Some(br) = line.br {
                self.write_line_break(br, line.char_width)?;
            } else if i + 1 < lines_count {
                self.write_new_line(line.char_width)?;
            }
        }
        Ok(())
    }

    /// Writes the specified HTML `entity` as if it were displayed as a single char.
    #[allow(clippy::option_if_let_else)] // false positive
    fn write_html_entity(&mut self, entity: &str) -> io::Result<()> {
        if let Some(splitter) = self.line_splitter_mut() {
            let lines = splitter.write_as_char(entity);
            self.write_lines(lines)
        } else {
            self.write_str(entity)
        }
    }

    /// Implements `io::Write::write()`.
    fn io_write(&mut self, buffer: &[u8], convert_spaces: bool) -> io::Result<usize> {
        let mut last_escape = 0;
        for (i, &byte) in buffer.iter().enumerate() {
            let escaped = match byte {
                b'>' => "&gt;",
                b'<' => "&lt;",
                b'&' => "&amp;",
                b' ' if convert_spaces => "\u{a0}", // non-breakable space
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
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum IndexOrRgb {
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

    pub fn indexed_color(index: u8) -> Self {
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

impl Default for LineSplitter {
    fn default() -> Self {
        Self {
            max_width: usize::MAX,
            current_width: 0,
        }
    }
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
            let char_width = self.current_width;
            self.current_width = 1;
            vec![
                Line {
                    text: "",
                    br: Some(LineBreak::Hard),
                    char_width,
                },
                Line {
                    text,
                    br: None,
                    char_width: 1,
                },
            ]
        } else {
            self.current_width += 1;
            vec![Line {
                text,
                br: None,
                char_width: self.current_width,
            }]
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
                    char_width: self.current_width,
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
            char_width: self.current_width,
        });
        output_lines
    }
}

#[derive(Debug, PartialEq)]
struct Line<'a> {
    text: &'a str,
    br: Option<LineBreak>,
    char_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineBreak {
    Hard,
}
