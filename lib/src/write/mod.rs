//! Rendering logic for terminal outputs.

use std::{io, mem, str};

use serde::Serialize;
use termcolor::{Color, ColorSpec, WriteColor};
use unicode_width::UnicodeWidthChar;

use crate::utils::RgbColor;

#[cfg(test)]
mod tests;

/*
/// HTML `<span>` / SVG `<tspan>` containing styling info.
#[derive(Debug, Default, Serialize)]
struct StyledSpan {
    classes: String,
    styles: String,
}

impl StyledSpan {
    fn new(spec: &ColorSpec, fg_property: &str) -> io::Result<Self> {
        let mut this = Self::default();
        if spec.bold() {
            this.push_class("bold");
        }
        if spec.dimmed() {
            this.push_class("dimmed");
        }
        if spec.italic() {
            this.push_class("italic");
        }
        if spec.underline() {
            this.push_class("underline");
        }

        if let Some(color) = spec.fg() {
            let color = IndexOrRgb::new(*color)?;
            this.set_fg(color, spec.intense(), &[fg_property]);
        }
        Ok(this)
    }

    fn push_class(&mut self, class: &str) {
        if !self.classes.is_empty() {
            self.classes.push(' ');
        }
        self.classes.push_str(class);
    }

    fn push_style(&mut self, prop: &str, value: &str) {
        if !self.styles.is_empty() {
            self.styles.push_str("; ");
        }
        self.styles.push_str(prop);
        self.styles.push_str(": ");
        self.styles.push_str(value);
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
                self.push_class(&fore_color_class);
            }
            IndexOrRgb::Rgb(r, g, b) => {
                for &property in fg_properties {
                    self.push_style(property, &format!("#{r:02x}{g:02x}{b:02x}"));
                }
            }
        }
    }
}
 */

/// Serializable `ColorSpec` representation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)] // makes serialization simpler
struct Style {
    #[serde(skip_serializing_if = "Style::is_false")]
    bold: bool,
    #[serde(skip_serializing_if = "Style::is_false")]
    italic: bool,
    #[serde(skip_serializing_if = "Style::is_false")]
    underline: bool,
    #[serde(skip_serializing_if = "Style::is_false")]
    dimmed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fg: Option<IndexOrRgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bg: Option<IndexOrRgb>,
}

impl Style {
    #[allow(clippy::trivially_copy_pass_by_ref)] // required by `serde`
    fn is_false(&val: &bool) -> bool {
        !val
    }

    fn new(spec: &ColorSpec) -> io::Result<Self> {
        let mut fg = spec.fg().copied().map(IndexOrRgb::new).transpose()?;
        let mut bg = spec.bg().copied().map(IndexOrRgb::new).transpose()?;
        if spec.intense() {
            // Switch to intense colors.
            if let Some(IndexOrRgb::Index(idx)) = &mut fg {
                *idx |= 8;
            }
            if let Some(IndexOrRgb::Index(idx)) = &mut bg {
                *idx |= 8;
            }
        }

        Ok(Self {
            bold: spec.bold(),
            italic: spec.italic(),
            underline: spec.underline(),
            dimmed: spec.dimmed(),
            fg,
            bg,
        })
    }
}

#[derive(Debug, Default, PartialEq, Serialize)]
struct StyledSpan {
    #[serde(flatten)]
    style: Style,
    text: String,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct StyledLine {
    spans: Vec<StyledSpan>,
    br: Option<LineBreak>,
}

impl StyledLine {
    fn push_str(&mut self, s: &str) {
        if self.spans.is_empty() {
            self.spans.push(StyledSpan::default());
        }
        self.spans.last_mut().unwrap().text.push_str(s);
    }

    fn write_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        let style = Style::new(spec)?;
        self.push_span(StyledSpan {
            style,
            text: String::new(),
        });
        Ok(())
    }

    fn push_span(&mut self, span: StyledSpan) {
        if let Some(last) = self.spans.last_mut() {
            if last.text.is_empty() {
                // Reuse the existing empty segment.
                *last = span;
                return;
            }
        }
        self.spans.push(span);
    }

    fn reset_color(&mut self) {
        self.push_span(StyledSpan::default());
    }

    fn trimmed(mut self) -> Self {
        if self.spans.last().is_some_and(|span| span.text.is_empty()) {
            self.spans.pop();
        }
        debug_assert!(self.spans.iter().all(|span| !span.text.is_empty()));
        self
    }
}

#[derive(Debug)]
pub(crate) struct LineWriter {
    lines: Vec<StyledLine>,
    current_line: StyledLine,
    current_style: Option<ColorSpec>,
    line_splitter: LineSplitter,
}

impl LineWriter {
    pub fn new(max_width: Option<usize>) -> Self {
        Self {
            lines: vec![],
            current_line: StyledLine::default(),
            current_style: None,
            line_splitter: max_width.map_or_else(LineSplitter::default, LineSplitter::new),
        }
    }

    fn write_color(&mut self, spec: ColorSpec) -> io::Result<()> {
        self.current_line.write_color(&spec)?;
        self.current_style = Some(spec);
        Ok(())
    }

    fn reset_inner(&mut self) {
        if self.current_style.is_some() {
            self.current_line.reset_color();
            self.current_style = None;
        }
    }

    pub fn into_lines(mut self) -> Vec<StyledLine> {
        if self.line_splitter.current_width > 0 {
            self.lines.push(mem::take(&mut self.current_line).trimmed());
        }
        self.lines
    }

    fn write_new_line(&mut self, br: Option<LineBreak>) -> io::Result<()> {
        let current_style = self.current_style.clone();
        self.reset_inner();

        let mut line = mem::take(&mut self.current_line).trimmed();
        line.br = br;
        self.lines.push(line);

        if let Some(spec) = current_style {
            self.write_color(spec)?;
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
            self.current_line.push_str(line.text);
            if i + 1 < lines_count || line.br.is_some() {
                self.write_new_line(line.br)?;
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

impl io::Write for LineWriter {
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

impl WriteColor for LineWriter {
    fn supports_color(&self) -> bool {
        true
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        debug_assert!(spec.reset());
        self.reset()?;
        if !spec.is_none() {
            self.write_color(spec.clone())?;
        }
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.reset_inner();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum IndexOrRgb {
    Index(u8),
    Rgb(RgbColor),
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
            Color::Rgb(r, g, b) => Self::Rgb(RgbColor(r, g, b)),
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
                Self::Rgb(RgbColor(r, g, b))
            }

            _ => {
                let gray = 10 * (index - 232) + 8;
                Self::Rgb(RgbColor(gray, gray, gray))
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
