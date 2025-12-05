//! Rendering logic for terminal outputs.

use std::{fmt, io, mem, str};

use serde::Serialize;
use termcolor::{Color, ColorSpec, WriteColor};
use unicode_width::UnicodeWidthChar;

pub(crate) use self::html::HtmlLine;
#[cfg(feature = "svg")]
pub(crate) use self::svg::SvgLine;

mod html;
#[cfg(feature = "svg")]
mod svg;
#[cfg(test)]
mod tests;

/// HTML `<span>` / SVG `<tspan>` containing styling info.
#[derive(Debug)]
pub(crate) struct StyledSpan {
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

    fn write_attrs(self, output: &mut String) {
        if !self.classes.is_empty() {
            output.push_str(" class=\"");
            for (i, class) in self.classes.iter().enumerate() {
                output.push_str(class);
                if i + 1 < self.classes.len() {
                    output.push(' ');
                }
            }
            output.push('\"');
        }
        if !self.styles.is_empty() {
            output.push_str(" style=\"");
            for (i, style) in self.styles.iter().enumerate() {
                output.push_str(style);
                if i + 1 < self.styles.len() {
                    output.push_str("; ");
                }
            }
            output.push_str(";\"");
        }
    }

    fn write_tag(self, output: &mut String, tag: &str) {
        output.push('<');
        output.push_str(tag);
        self.write_attrs(output);
        output.push('>');
    }
}

pub(crate) trait StyledLine: Default + AsMut<String> {
    fn write_color(&mut self, spec: &ColorSpec, start_pos: usize) -> io::Result<()>;
    fn reset_color(&mut self, prev_spec: &ColorSpec, current_width: usize);
    fn set_br(&mut self, br: Option<LineBreak>);
}

#[derive(Debug)]
pub(crate) struct GenericWriter<L> {
    lines: Vec<L>,
    current_line: L,
    current_style: Option<ColorSpec>,
    line_splitter: LineSplitter,
}

impl<L: StyledLine> GenericWriter<L> {
    pub fn new(max_width: Option<usize>) -> Self {
        Self {
            lines: vec![],
            current_line: L::default(),
            current_style: None,
            line_splitter: max_width.map_or_else(LineSplitter::default, LineSplitter::new),
        }
    }

    fn write_color(&mut self, spec: ColorSpec, start_pos: usize) -> io::Result<()> {
        self.current_line.write_color(&spec, start_pos)?;
        self.current_style = Some(spec);
        Ok(())
    }

    fn reset_inner(&mut self, line_width: Option<usize>) {
        if let Some(spec) = &self.current_style {
            let line_width = line_width.unwrap_or(self.line_splitter.current_width);
            self.current_line.reset_color(spec, line_width);
            self.current_style = None;
        }
    }

    pub fn into_lines(mut self) -> Vec<L> {
        if self.line_splitter.current_width > 0 {
            self.lines.push(mem::take(&mut self.current_line));
        }
        self.lines
    }

    fn write_new_line(&mut self, char_width: usize, br: Option<LineBreak>) -> io::Result<()> {
        let current_style = self.current_style.clone();
        self.reset_inner(Some(char_width));

        let mut line = mem::take(&mut self.current_line);
        line.set_br(br);
        self.lines.push(line);

        if let Some(spec) = current_style {
            self.write_color(spec, 0)?;
        }
        Ok(())
    }

    /// Writes the specified text displayed to the user that should be subjected to wrapping.
    fn write_text(&mut self, s: &str) -> io::Result<()> {
        let lines = self.line_splitter.split_lines(s);
        self.write_lines(lines)
    }

    fn write_lines(&mut self, lines: Vec<Line<'_>>) -> io::Result<()> {
        let lines_count = lines.len();
        let it = lines.into_iter().enumerate();
        for (i, line) in it {
            self.current_line.as_mut().push_str(line.text);
            if i + 1 < lines_count || line.br.is_some() {
                self.write_new_line(line.char_width, line.br)?;
            }
        }
        Ok(())
    }

    /// Writes the specified HTML `entity` as if it were displayed as a single char.
    fn write_html_entity(&mut self, entity: &str) -> io::Result<()> {
        let lines = self.line_splitter.write_as_char(entity);
        self.write_lines(lines)
    }
}

impl<L: StyledLine> io::Write for GenericWriter<L> {
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

impl<L: StyledLine> WriteColor for GenericWriter<L> {
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
        self.reset_inner(None);
        Ok(())
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
            _ => return Err(io::Error::other("Unsupported color")),
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LineBreak {
    Hard,
}
