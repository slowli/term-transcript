//! Comparing `Styled` instances by styling.

use core::{
    cmp::{self, Ordering},
    fmt,
    iter::{self, Peekable},
    mem,
};

use anstyle::{AnsiColor, Color, Style};
use unicode_width::UnicodeWidthStr;

use crate::{StyledSpan, rich_parser::RichStyle, types::StyledStr};

#[cfg(test)]
mod tests;

impl StyledSpan {
    /// Writes a single plaintext `line` to `out` using styles from `spans_iter`.
    fn write_line<'a, I>(
        formatter: &mut fmt::Formatter<'_>,
        spans_iter: &mut Peekable<I>,
        line_start: usize,
        line: &str,
    ) -> fmt::Result
    where
        I: Iterator<Item = (usize, &'a Self)>,
    {
        let mut pos = 0;
        while pos < line.len() {
            let &(span_start, span) = spans_iter.peek().expect("spans ended before lines");
            let span_len = span.len - line_start.saturating_sub(span_start);

            let span_end = cmp::min(pos + span_len, line.len());
            write!(
                formatter,
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
        writeln!(formatter)
    }
}

#[derive(Debug)]
struct DiffStyleSpan {
    start: usize,
    len: usize,
    lhs_color_spec: Style,
    rhs_color_spec: Style,
}

const STYLE_WIDTH: usize = 25;

#[derive(Debug)]
pub struct StyleDiff<'a> {
    text: &'a str,
    lhs_spans: &'a [StyledSpan],
    differing_spans: Vec<DiffStyleSpan>,
}

// FIXME: document
impl fmt::Display for StyleDiff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if formatter.alternate() {
            self.write_as_table(formatter)
        } else {
            self.highlight_text(formatter)
        }
    }
}

impl<'a> StyleDiff<'a> {
    /// # Panics
    ///
    /// Panics if `lhs` and `rhs` have differing lengths.
    pub fn new(lhs: StyledStr<'a>, rhs: StyledStr<'a>) -> Self {
        assert_eq!(
            lhs.text.len(),
            rhs.text.len(),
            "Compared strings must have same length"
        );

        let mut this = Self {
            text: lhs.text,
            lhs_spans: lhs.spans,
            differing_spans: Vec::new(),
        };
        let mut pos = 0;
        let mut lhs_iter = lhs.spans.iter().copied();
        let Some(mut lhs_span) = lhs_iter.next() else {
            return this;
        };
        let mut rhs_iter = rhs.spans.iter().copied();
        let Some(mut rhs_span) = rhs_iter.next() else {
            return this;
        };

        loop {
            let common_len = cmp::min(lhs_span.len, rhs_span.len);

            // Record a diff span if the color specs differ.
            if lhs_span.style != rhs_span.style {
                this.differing_spans.push(DiffStyleSpan {
                    start: pos,
                    len: common_len,
                    lhs_color_spec: lhs_span.style,
                    rhs_color_spec: rhs_span.style,
                });
            }

            pos += common_len;

            match lhs_span.len.cmp(&rhs_span.len) {
                Ordering::Less => {
                    rhs_span.len -= lhs_span.len;
                    lhs_span = lhs_iter.next().unwrap();
                    // ^ `unwrap()` here and below are safe; we've checked that
                    // `lhs` and `rhs` contain same total span coverage.
                }
                Ordering::Greater => {
                    lhs_span.len -= rhs_span.len;
                    rhs_span = rhs_iter.next().unwrap();
                }
                Ordering::Equal => {
                    lhs_span = match lhs_iter.next() {
                        Some(span) => span,
                        None => return this,
                    };
                    rhs_span = match rhs_iter.next() {
                        Some(span) => span,
                        None => return this,
                    };
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.differing_spans.is_empty()
    }

    /// Highlights this diff on the specified `text` which has styling set with `color_spans`.
    fn highlight_text(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SIDELINE_HL: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));

        let highlights = HighlightedSpan::new(&self.differing_spans);
        let mut highlights = highlights.iter().copied().peekable();
        let mut line_start = 0;

        // Spans together with their starting index
        let mut span_start = 0;
        let mut color_spans = self
            .lhs_spans
            .iter()
            .map(move |span| {
                let prev_start = span_start;
                span_start += span.len;
                (prev_start, span)
            })
            .peekable();

        for line in self.text.split('\n') {
            let line_contains_spans = highlights
                .peek()
                .is_some_and(|span| span.start <= line_start + line.len());

            if line_contains_spans {
                write!(formatter, "{SIDELINE_HL}> {SIDELINE_HL:#}")?;
                StyledSpan::write_line(formatter, &mut color_spans, line_start, line)?;
                write!(formatter, "{SIDELINE_HL}> {SIDELINE_HL:#}")?;
                Self::highlight_line(formatter, &mut highlights, line_start, line)?;
            } else {
                write!(formatter, "= ")?;
                StyledSpan::write_line(formatter, &mut color_spans, line_start, line)?;
            }
            line_start += line.len() + 1;
        }
        Ok(())
    }

    fn highlight_line<I>(
        out: &mut fmt::Formatter<'_>,
        spans_iter: &mut Peekable<I>,
        line_offset: usize,
        line: &str,
    ) -> fmt::Result
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

    pub(crate) fn write_as_table(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        const TABLE_HEAD: Style = Style::new().bold();
        const POS_WIDTH: usize = 10;

        // Write table header.
        writeln!(
            formatter,
            "{TABLE_HEAD}{pos:^POS_WIDTH$} {lhs:^STYLE_WIDTH$} {rhs:^STYLE_WIDTH$}",
            pos = "Positions",
            lhs = "Left style",
            rhs = "Right style"
        )?;
        writeln!(
            formatter,
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
                    write!(formatter, "{pos:>POS_WIDTH$} ")?;
                } else {
                    write!(formatter, "{:>POS_WIDTH$} ", "")?;
                }
                writeln!(
                    formatter,
                    "{lhs_style}{lhs_line:^STYLE_WIDTH$}{lhs_style:#} \
                     {rhs_style}{rhs_line:^STYLE_WIDTH$}{rhs_style:#}"
                )?;
            }
        }

        Ok(())
    }

    /// Writes `color_spec` in human-readable format.
    fn write_style(style: &Style) -> Vec<String> {
        let mut tokens = RichStyle(style).tokens();
        if tokens.is_empty() {
            tokens.push("(none)".into());
        }

        let mut lines = vec![];
        let mut current_line = String::new();
        for token in tokens {
            // We can use `len()` because all text is ASCII.
            if !current_line.is_empty() && current_line.len() + 1 + token.len() > STYLE_WIDTH {
                lines.push(mem::take(&mut current_line));
            }
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(&token);
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
        lines
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
        let mut style = Style::new();
        match self {
            Self::Main => {
                style = style
                    .fg_color(Some(AnsiColor::White.into()))
                    .bg_color(Some(AnsiColor::Red.into()));
            }
            Self::Aux => {
                style = style
                    .fg_color(Some(AnsiColor::Black.into()))
                    .bg_color(Some(AnsiColor::Yellow.into()));
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
    fn new(differing_spans: &[DiffStyleSpan]) -> Vec<Self> {
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
