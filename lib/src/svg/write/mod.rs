//! Rendering logic for terminal outputs.

use std::{io, mem, str};

use serde::Serialize;
use unicode_width::UnicodeWidthChar;

use crate::style::{Style, StyledSpan, WriteStyled};

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Serialize)]
pub(crate) struct StyledLine {
    pub(crate) spans: Vec<StyledSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    br: Option<LineBreak>,
}

impl StyledLine {
    fn push_str(&mut self, s: &str) {
        if self.spans.is_empty() {
            self.spans.push(StyledSpan::default());
        }
        self.spans.last_mut().unwrap().text.push_str(s);
    }

    fn write_style(&mut self, style: Style) {
        self.push_span(StyledSpan {
            style,
            text: String::new(),
        });
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
    current_style: Option<Style>,
    line_splitter: LineSplitter,
}

impl LineWriter {
    pub(crate) fn new(max_width: Option<usize>) -> Self {
        Self {
            lines: vec![],
            current_line: StyledLine::default(),
            current_style: None,
            line_splitter: max_width.map_or_else(LineSplitter::default, LineSplitter::new),
        }
    }

    fn write_style_inner(&mut self, mut style: Style) {
        style.normalize();
        self.current_line.write_style(style);
        self.current_style = Some(style);
    }

    fn reset_inner(&mut self) {
        if self.current_style.is_some() {
            self.current_line.reset_color();
            self.current_style = None;
        }
    }

    pub(crate) fn into_lines(mut self) -> Vec<StyledLine> {
        if self.line_splitter.current_width > 0 {
            self.lines.push(mem::take(&mut self.current_line).trimmed());
        }
        self.lines
    }

    fn write_new_line(&mut self, br: Option<LineBreak>) {
        let current_style = self.current_style;
        self.reset_inner();

        let mut line = mem::take(&mut self.current_line).trimmed();
        line.br = br;
        self.lines.push(line);

        if let Some(spec) = current_style {
            self.write_style_inner(spec);
        }
    }

    fn write_lines(&mut self, lines: Vec<Line<'_>>) {
        let lines_count = lines.len();
        let it = lines.into_iter().enumerate();
        for (i, line) in it {
            self.current_line.push_str(line.text);
            if i + 1 < lines_count || line.br.is_some() {
                self.write_new_line(line.br);
            }
        }
    }
}

impl WriteStyled for LineWriter {
    fn write_style(&mut self, style: &Style) -> io::Result<()> {
        self.reset_inner();
        if !style.is_none() {
            self.write_style_inner(*style);
        }
        Ok(())
    }

    fn write_text(&mut self, text: &str) -> io::Result<()> {
        let lines = self.line_splitter.split_lines(text);
        self.write_lines(lines);
        Ok(())
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
