use std::{
    cmp::{self, Ordering},
    io,
    io::Write,
    iter::{self, Peekable},
    mem,
};

use unicode_width::UnicodeWidthStr;

#[cfg(test)]
mod tests;

use crate::{
    TermError,
    style::{Color, RgbColor, Style, StyledSpan, WriteStyled},
    term::TermOutputParser,
};

impl StyledSpan<usize> {
    pub(crate) fn parse(ansi_text: &str) -> Result<Vec<Self>, TermError> {
        let mut spans = ColorSpansWriter::default();
        TermOutputParser::new(&mut spans).parse(ansi_text.as_bytes())?;
        Ok(spans.shrink().spans)
    }

    pub(crate) fn write_colorized(
        spans: &[Self],
        out: &mut impl Write,
        plaintext: &str,
    ) -> io::Result<()> {
        debug_assert_eq!(
            spans.iter().map(|span| span.text).sum::<usize>(),
            plaintext.len()
        );
        let mut pos = 0;
        for span in spans {
            write!(
                out,
                "{style}{}{style:#}",
                &plaintext[pos..pos + span.text],
                style = span.style
            )?;
            pos += span.text;
        }
        Ok(())
    }

    /// Writes a single plaintext `line` to `out` using styles from `spans_iter`.
    fn write_line<'a, I>(
        spans_iter: &mut Peekable<I>,
        out: &mut impl Write,
        line_start: usize,
        line: &str,
    ) -> io::Result<()>
    where
        I: Iterator<Item = (usize, &'a Self)>,
    {
        let mut pos = 0;
        while pos < line.len() {
            let &(span_start, span) = spans_iter.peek().expect("spans ended before lines");
            let span_len = span.text - line_start.saturating_sub(span_start);

            let span_end = cmp::min(pos + span_len, line.len());
            write!(
                out,
                "{style}{}{style:#}",
                &line[pos..span_end],
                style = span.style
            )?;
            if span_end == pos + span_len {
                // The span has ended, can proceed to the next one.
                spans_iter.next();
            }
            pos += span_len;
        }
        writeln!(out)
    }
}

/// `Write` / `WriteColor` implementation recording `ColorSpan`s for the input text.
#[derive(Debug, Default)]
pub(crate) struct ColorSpansWriter {
    spans: Vec<StyledSpan<usize>>,
    style: Style,
}

impl ColorSpansWriter {
    /// Unites sequential spans with the same color spec.
    fn shrink(self) -> Self {
        let mut shrunk_spans = Vec::<StyledSpan<_>>::with_capacity(self.spans.len());
        for span in self.spans {
            if let Some(last_span) = shrunk_spans.last_mut() {
                if last_span.style == span.style {
                    last_span.text += span.text;
                } else {
                    shrunk_spans.push(span);
                }
            } else {
                shrunk_spans.push(span);
            }
        }

        Self {
            spans: shrunk_spans,
            style: self.style,
        }
    }

    pub(crate) fn into_inner(self) -> Vec<StyledSpan<usize>> {
        self.shrink().spans
    }
}

impl WriteStyled for ColorSpansWriter {
    fn write_style(&mut self, style: &Style) -> io::Result<()> {
        let mut normalized_style = *style;
        normalized_style.normalize();
        self.style = normalized_style;
        Ok(())
    }

    fn write_text(&mut self, text: &str) -> io::Result<()> {
        // Break styling on newlines because it will be broken in the parsed transcripts.
        let lines = text.split('\n');
        let mut pos = 0;
        self.spans.extend(lines.flat_map(|line| {
            let mut new_spans = vec![];
            if !line.is_empty() {
                new_spans.push(StyledSpan {
                    style: self.style,
                    text: line.len(),
                });
            }
            pos += line.len();
            if pos < text.len() {
                new_spans.push(StyledSpan {
                    style: Style::default(),
                    text: 1,
                });
                pos += 1;
            }
            new_spans
        }));
        Ok(())
    }
}

#[derive(Debug)]
struct DiffColorSpan {
    start: usize,
    len: usize,
    lhs_color_spec: Style,
    rhs_color_spec: Style,
}

const STYLE_WIDTH: usize = 25;

#[derive(Debug, Default)]
pub(crate) struct ColorDiff {
    differing_spans: Vec<DiffColorSpan>,
}

impl ColorDiff {
    pub(crate) fn new(lhs: &[StyledSpan<usize>], rhs: &[StyledSpan<usize>]) -> Self {
        debug_assert_eq!(
            lhs.iter().map(|span| span.text).sum::<usize>(),
            rhs.iter().map(|span| span.text).sum::<usize>(),
            "Spans {lhs:?} and {rhs:?} must have equal total covered length"
        );

        let mut diff = Self::default();
        let mut pos = 0;
        let mut lhs_iter = lhs.iter().copied();
        let Some(mut lhs_span) = lhs_iter.next() else {
            return diff;
        };
        let mut rhs_iter = rhs.iter().copied();
        let Some(mut rhs_span) = rhs_iter.next() else {
            return diff;
        };

        loop {
            let common_len = cmp::min(lhs_span.text, rhs_span.text);

            // Record a diff span if the color specs differ.
            if lhs_span.style != rhs_span.style {
                diff.differing_spans.push(DiffColorSpan {
                    start: pos,
                    len: common_len,
                    lhs_color_spec: lhs_span.style,
                    rhs_color_spec: rhs_span.style,
                });
            }

            pos += common_len;

            match lhs_span.text.cmp(&rhs_span.text) {
                Ordering::Less => {
                    rhs_span.text -= lhs_span.text;
                    lhs_span = lhs_iter.next().unwrap();
                    // ^ `unwrap()` here and below are safe; we've checked that
                    // `lhs` and `rhs` contain same total span coverage.
                }
                Ordering::Greater => {
                    lhs_span.text -= rhs_span.text;
                    rhs_span = rhs_iter.next().unwrap();
                }
                Ordering::Equal => {
                    lhs_span = match lhs_iter.next() {
                        Some(span) => span,
                        None => return diff,
                    };
                    rhs_span = match rhs_iter.next() {
                        Some(span) => span,
                        None => return diff,
                    };
                }
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.differing_spans.is_empty()
    }

    /// Highlights this diff on the specified `text` which has styling set with `color_spans`.
    pub(crate) fn highlight_text(
        &self,
        out: &mut impl Write,
        text: &str,
        color_spans: &[StyledSpan<usize>],
    ) -> io::Result<()> {
        const SIDELINE_HL: Style = Style {
            fg: Some(Color::RED),
            ..Style::NONE
        };

        let highlights = HighlightedSpan::new(&self.differing_spans);
        let mut highlights = highlights.iter().copied().peekable();
        let mut line_start = 0;

        // Spans together with their starting index
        let mut span_start = 0;
        let mut color_spans = color_spans
            .iter()
            .map(move |span| {
                let prev_start = span_start;
                span_start += span.text;
                (prev_start, span)
            })
            .peekable();

        for line in text.split('\n') {
            let line_contains_spans = highlights
                .peek()
                .is_some_and(|span| span.start <= line_start + line.len());

            if line_contains_spans {
                write!(out, "{SIDELINE_HL}> {SIDELINE_HL:#}")?;
                StyledSpan::write_line(&mut color_spans, out, line_start, line)?;
                write!(out, "{SIDELINE_HL}> {SIDELINE_HL:#}")?;
                Self::highlight_line(out, &mut highlights, line_start, line)?;
            } else {
                write!(out, "= ")?;
                StyledSpan::write_line(&mut color_spans, out, line_start, line)?;
            }
            line_start += line.len() + 1;
        }
        Ok(())
    }

    fn highlight_line<I>(
        out: &mut impl Write,
        spans_iter: &mut Peekable<I>,
        line_offset: usize,
        line: &str,
    ) -> io::Result<()>
    where
        I: Iterator<Item = HighlightedSpan>,
    {
        let line_len = line.len();
        let mut line_pos = 0;

        while line_pos < line_len {
            let Some(span) = spans_iter.peek() else {
                break;
            };
            let span_start = span.start.saturating_sub(line_offset);
            if span_start >= line_len {
                break;
            }
            let span_end = cmp::min(span.start + span.len - line_offset, line_len);

            if span_start > line_pos {
                let spaces = " ".repeat(line[line_pos..span_start].width());
                write!(out, "{spaces}")?;
            }

            let ch = span.kind.underline_char();
            let underline: String =
                iter::repeat_n(ch, line[span_start..span_end].width()).collect();
            let hl = span.kind.highlight_style();
            write!(out, "{hl}{underline}{hl:#}")?;

            line_pos = span_end;
            if span.start + span.len <= line_offset + line_len {
                // Span is finished on this line; can proceed to the next one.
                spans_iter.next();
            }
        }
        writeln!(out)
    }

    pub(crate) fn write_as_table(&self, out: &mut impl Write) -> io::Result<()> {
        const TABLE_HEAD: Style = Style {
            bold: true,
            ..Style::NONE
        };
        const POS_WIDTH: usize = 10;

        // Write table header.
        writeln!(
            out,
            "{TABLE_HEAD}{pos:^POS_WIDTH$} {lhs:^STYLE_WIDTH$} {rhs:^STYLE_WIDTH$}",
            pos = "Positions",
            lhs = "Expected style",
            rhs = "Actual style"
        )?;
        writeln!(
            out,
            "{pos:=>POS_WIDTH$} {lhs:=>STYLE_WIDTH$} {rhs:=>STYLE_WIDTH$}{TABLE_HEAD:#}",
            pos = "",
            lhs = "",
            rhs = ""
        )?;

        // Write table itself.
        for differing_span in &self.differing_spans {
            let lhs_style = &differing_span.lhs_color_spec;
            let mut lhs_lines = Self::write_style(lhs_style);
            let rhs_style = &differing_span.rhs_color_spec;
            let mut rhs_lines = Self::write_style(rhs_style);
            if lhs_lines.len() < rhs_lines.len() {
                lhs_lines.resize_with(rhs_lines.len(), String::new);
            } else {
                rhs_lines.resize_with(lhs_lines.len(), String::new);
            }

            for (i, (lhs_line, rhs_line)) in lhs_lines.into_iter().zip(rhs_lines).enumerate() {
                if i == 0 {
                    let start = differing_span.start;
                    let end = start + differing_span.len;
                    let pos = format!("{start}..{end}");
                    write!(out, "{pos:>POS_WIDTH$} ")?;
                } else {
                    write!(out, "{:>POS_WIDTH$} ", "")?;
                }
                writeln!(
                    out,
                    "{lhs_style}{lhs_line:^STYLE_WIDTH$}{lhs_style:#} \
                     {rhs_style}{rhs_line:^STYLE_WIDTH$}{rhs_style:#}"
                )?;
            }
        }

        Ok(())
    }

    /// Writes `color_spec` in human-readable format.
    fn write_style(style: &Style) -> Vec<String> {
        let mut tags = vec![];
        if let Some(color) = style.fg {
            tags.push(format!("fg:{}", Self::color_to_string(color)));
        }
        if let Some(color) = style.bg {
            tags.push(format!("bg:{}", Self::color_to_string(color)));
        }
        if style.bold {
            tags.push("bold".to_owned());
        }
        if style.italic {
            tags.push("italic".to_owned());
        }
        if style.underline {
            tags.push("underline".to_owned());
        }
        if style.strikethrough {
            tags.push("strikethrough".to_owned());
        }
        if style.dimmed {
            tags.push("dimmed".to_owned());
        }
        if style.blink {
            tags.push("blink".to_owned());
        }
        if style.concealed {
            tags.push("concealed".to_owned());
        }

        if tags.is_empty() {
            tags.push("(none)".to_owned());
        }

        let mut lines = vec![];
        let mut current_line = String::new();
        for tag in tags {
            // We can use `len()` because all text is ASCII.
            if !current_line.is_empty() && current_line.len() + 1 + tag.len() > STYLE_WIDTH {
                lines.push(mem::take(&mut current_line));
            }
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(&tag);
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
        lines
    }

    fn color_to_string(color: Color) -> String {
        match color {
            Color::Index(0) => "black".to_owned(),
            Color::Index(1) => "red".to_owned(),
            Color::Index(2) => "green".to_owned(),
            Color::Index(3) => "yellow".to_owned(),
            Color::Index(4) => "blue".to_owned(),
            Color::Index(5) => "magenta".to_owned(),
            Color::Index(6) => "cyan".to_owned(),
            Color::Index(7) => "white".to_owned(),

            Color::Index(8) => "black*".to_owned(),
            Color::Index(9) => "red*".to_owned(),
            Color::Index(10) => "green*".to_owned(),
            Color::Index(11) => "yellow*".to_owned(),
            Color::Index(12) => "blue*".to_owned(),
            Color::Index(13) => "magenta*".to_owned(),
            Color::Index(14) => "cyan*".to_owned(),
            Color::Index(15) => "white*".to_owned(),

            Color::Rgb(RgbColor(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),

            Color::Index(_) => unreachable!(), // must be transformed during color normalization
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SpanHighlightKind {
    Main,
    Aux,
}

impl SpanHighlightKind {
    fn underline_char(self) -> char {
        match self {
            Self::Main => '^',
            Self::Aux => '!',
        }
    }

    fn highlight_style(self) -> Style {
        let mut style = Style::default();
        match self {
            Self::Main => {
                style.fg = Some(Color::WHITE);
                style.bg = Some(Color::RED);
            }
            Self::Aux => {
                style.fg = Some(Color::BLACK);
                style.bg = Some(Color::YELLOW);
            }
        }
        style
    }
}

#[derive(Debug, Clone, Copy)]
struct HighlightedSpan {
    start: usize,
    len: usize,
    kind: SpanHighlightKind,
}

impl HighlightedSpan {
    fn new(differing_spans: &[DiffColorSpan]) -> Vec<Self> {
        let mut sequential_span_count = 1;
        let span_highlights = differing_spans.windows(2).map(|window| match window {
            [prev, next] => {
                if prev.start + prev.len == next.start {
                    sequential_span_count += 1;
                } else {
                    sequential_span_count = 1;
                }
                if sequential_span_count % 2 == 0 {
                    SpanHighlightKind::Aux
                } else {
                    SpanHighlightKind::Main
                }
            }
            _ => unreachable!(),
        });

        iter::once(SpanHighlightKind::Main)
            .chain(span_highlights)
            .zip(differing_spans)
            .map(|(kind, span)| Self {
                start: span.start,
                len: span.len,
                kind,
            })
            .collect()
    }
}
