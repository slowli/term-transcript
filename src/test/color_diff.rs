use termcolor::{Color, ColorSpec, WriteColor};

use std::{
    cmp::{self, Ordering},
    io,
    iter::{self, Peekable},
};

use crate::{html::IndexOrRgb, term::TermOutputParser, TermError};

#[derive(Debug, Clone)]
pub(crate) struct ColorSpan {
    len: usize,
    color_spec: ColorSpec,
}

impl ColorSpan {
    pub fn parse(ansi_text: &str) -> Result<Vec<Self>, TermError> {
        let mut spans = ColorSpansWriter::default();
        TermOutputParser::new(&mut spans).parse(ansi_text.as_bytes())?;
        Ok(spans.shrink().spans)
    }

    pub fn write_colorized(
        spans: &[Self],
        out: &mut impl WriteColor,
        plaintext: &str,
    ) -> io::Result<()> {
        debug_assert_eq!(
            spans.iter().map(|span| span.len).sum::<usize>(),
            plaintext.len()
        );
        let mut pos = 0;
        for span in spans {
            out.set_color(&span.color_spec)?;
            write!(out, "{}", &plaintext[pos..pos + span.len])?;
            pos += span.len;
        }
        Ok(())
    }

    /// Writes a single plaintext `line` to `out` using styles from `styles_iter`.
    /// `first_span_len` can be used to overwrite the effective length of the first span;
    /// this is used when calling this method multiple times in succession.
    fn write_line<'a, I: Iterator<Item = (usize, &'a Self)>>(
        spans_iter: &mut Peekable<I>,
        out: &mut impl WriteColor,
        line_start: usize,
        line: &str,
    ) -> io::Result<()> {
        let mut pos = 0;
        while pos < line.len() {
            let &(span_start, span) = spans_iter.peek().expect("spans ended before lines");
            let span_len = span.len - line_start.saturating_sub(span_start);

            let span_end = cmp::min(pos + span_len, line.len());
            out.set_color(&span.color_spec)?;
            write!(out, "{}", &line[pos..span_end])?;
            if span_end == pos + span_len {
                // The span has ended, can proceed to the next one.
                spans_iter.next();
            }
            pos += span_len;
        }
        out.reset()?;
        writeln!(out)
    }
}

/// `Write` / `WriteColor` implementation recording `ColorSpan`s for the input text.
#[derive(Debug, Default)]
pub(crate) struct ColorSpansWriter {
    spans: Vec<ColorSpan>,
    color_spec: ColorSpec,
}

impl ColorSpansWriter {
    fn normalize_spec(mut spec: ColorSpec) -> ColorSpec {
        if let Some(color) = spec.fg().copied() {
            spec.set_fg(Some(Self::normalize_color(color)));
        }
        if let Some(color) = spec.bg().copied() {
            spec.set_bg(Some(Self::normalize_color(color)));
        }
        spec
    }

    fn normalize_color(color: Color) -> Color {
        match color {
            Color::Ansi256(0) => Color::Black,
            Color::Ansi256(1) => Color::Red,
            Color::Ansi256(2) => Color::Green,
            Color::Ansi256(3) => Color::Yellow,
            Color::Ansi256(4) => Color::Blue,
            Color::Ansi256(5) => Color::Magenta,
            Color::Ansi256(6) => Color::Cyan,
            Color::Ansi256(7) => Color::White,

            Color::Ansi256(index) if index >= 16 => match IndexOrRgb::indexed_color(index) {
                IndexOrRgb::Rgb(r, g, b) => Color::Rgb(r, g, b),
                IndexOrRgb::Index(_) => color,
            },

            _ => color,
        }
    }

    /// Unites sequential spans with the same color spec.
    fn shrink(self) -> Self {
        let mut shrunk_spans = Vec::<ColorSpan>::with_capacity(self.spans.len());
        for span in self.spans {
            if let Some(last_span) = shrunk_spans.last_mut() {
                if last_span.color_spec == span.color_spec {
                    last_span.len += span.len;
                } else {
                    shrunk_spans.push(span);
                }
            } else {
                shrunk_spans.push(span);
            }
        }

        Self {
            spans: shrunk_spans,
            color_spec: self.color_spec,
        }
    }

    pub fn into_inner(self) -> Vec<ColorSpan> {
        self.shrink().spans
    }
}

impl io::Write for ColorSpansWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.spans.push(ColorSpan {
            len: buffer.len(),
            color_spec: self.color_spec.clone(),
        });
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteColor for ColorSpansWriter {
    fn supports_color(&self) -> bool {
        true
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        debug_assert!(spec.reset());
        self.color_spec = Self::normalize_spec(spec.clone());
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.color_spec = ColorSpec::new();
        Ok(())
    }
}

#[derive(Debug)]
struct DiffColorSpan {
    start: usize,
    len: usize,
    lhs_color_spec: ColorSpec,
    rhs_color_spec: ColorSpec,
}

#[derive(Debug, Default)]
pub(crate) struct ColorDiff {
    differing_spans: Vec<DiffColorSpan>,
}

impl ColorDiff {
    pub fn new(lhs: &[ColorSpan], rhs: &[ColorSpan]) -> Self {
        debug_assert_eq!(
            lhs.iter().map(|span| span.len).sum::<usize>(),
            rhs.iter().map(|span| span.len).sum::<usize>(),
            "Spans {:?} and {:?} must have equal total covered length",
            lhs,
            rhs
        );

        let mut diff = Self::default();
        let mut pos = 0;
        let mut lhs_iter = lhs.iter().cloned();
        let mut lhs_span = match lhs_iter.next() {
            Some(span) => span,
            None => return diff,
        };
        let mut rhs_iter = rhs.iter().cloned();
        let mut rhs_span = match rhs_iter.next() {
            Some(span) => span,
            None => return diff,
        };

        loop {
            let common_len = cmp::min(lhs_span.len, rhs_span.len);

            // Record a diff span if the color specs differ.
            if lhs_span.color_spec != rhs_span.color_spec {
                diff.differing_spans.push(DiffColorSpan {
                    start: pos,
                    len: common_len,
                    lhs_color_spec: lhs_span.color_spec.clone(),
                    rhs_color_spec: rhs_span.color_spec.clone(),
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

    pub fn is_empty(&self) -> bool {
        self.differing_spans.is_empty()
    }

    /// Highlights this diff on the specified `text` which has styling set with `color_spans`.
    pub fn highlight_text(
        &self,
        out: &mut impl WriteColor,
        text: &str,
        color_spans: &[ColorSpan],
    ) -> io::Result<()> {
        let mut sideline_hl = ColorSpec::new();
        sideline_hl.set_fg(Some(Color::Red));

        let highlights = HighlightedSpan::new(&self.differing_spans);
        let mut highlights = highlights.iter().copied().peekable();
        let mut line_start = 0;

        // Spans together with their starting index
        let mut span_start = 0;
        let mut color_spans = color_spans
            .iter()
            .map(move |span| {
                let prev_start = span_start;
                span_start += span.len;
                (prev_start, span)
            })
            .peekable();

        for line in text.split('\n') {
            let line_contains_spans = highlights
                .peek()
                .map_or(false, |span| span.start <= line_start + line.len());

            if line_contains_spans {
                out.set_color(&sideline_hl)?;
                write!(out, "> ")?;
                out.reset()?;
                ColorSpan::write_line(&mut color_spans, out, line_start, line)?;
                out.set_color(&sideline_hl)?;
                write!(out, "> ")?;
                out.reset()?;
                Self::highlight_line(out, &mut highlights, line_start, line.len())?;
            } else {
                write!(out, "= ")?;
                ColorSpan::write_line(&mut color_spans, out, line_start, line)?;
            }
            line_start += line.len() + 1;
        }
        Ok(())
    }

    fn highlight_line<I: Iterator<Item = HighlightedSpan>>(
        out: &mut impl WriteColor,
        spans_iter: &mut Peekable<I>,
        line_offset: usize,
        line_len: usize,
    ) -> io::Result<()> {
        let mut line_pos = 0;
        while line_pos < line_len {
            let span = if let Some(span) = spans_iter.peek() {
                span
            } else {
                break;
            };
            let span_start = span.start.saturating_sub(line_offset);
            if span_start >= line_len {
                break;
            }
            let span_end = cmp::min(span.start + span.len - line_offset, line_len);

            if span_start > line_pos {
                let spaces: String = " ".repeat(span_start - line_pos);
                write!(out, "{}", spaces)?;
            }

            let ch = span.kind.underline_char();
            let underline: String = iter::repeat(ch).take(span_end - span_start).collect();
            out.set_color(&span.kind.highlight_spec())?;
            write!(out, "{}", underline)?;
            out.reset()?;

            line_pos = span_end;
            if span.start + span.len <= line_offset + line_len {
                // Span is finished on this line; can proceed to the next one.
                spans_iter.next();
            }
        }
        writeln!(out)
    }

    pub fn write_as_table(&self, out: &mut impl WriteColor) -> io::Result<()> {
        const POS_WIDTH: usize = 10;
        const STYLE_WIDTH: usize = 22; // `buid magenta*/magenta*`

        // Write table header.
        let mut table_header_spec = ColorSpec::new();
        table_header_spec.set_bold(true);
        table_header_spec.set_intense(true);
        out.set_color(&table_header_spec)?;
        writeln!(
            out,
            "{pos:^pos_width$} {lhs:^style_width$} {rhs:^style_width$}",
            pos_width = POS_WIDTH,
            style_width = STYLE_WIDTH,
            pos = "Positions",
            lhs = "Expected style",
            rhs = "Actual style"
        )?;
        writeln!(
            out,
            "{pos:=>pos_width$} {lhs:=>style_width$} {rhs:=>style_width$}",
            pos_width = POS_WIDTH,
            style_width = STYLE_WIDTH,
            pos = "",
            lhs = "",
            rhs = ""
        )?;
        out.reset()?;

        // Write table itself.
        for differing_span in &self.differing_spans {
            let start = differing_span.start;
            let end = start + differing_span.len;
            write!(
                out,
                "{pos:>pos_width$} ",
                pos_width = POS_WIDTH,
                pos = format!("{}..{}", start, end),
            )?;

            Self::write_color_spec(out, &differing_span.lhs_color_spec)?;
            out.write_all(b" ")?;
            Self::write_color_spec(out, &differing_span.rhs_color_spec)?;
            writeln!(out)?;
        }

        Ok(())
    }

    /// Writes `color_spec` in human-readable format.
    fn write_color_spec(out: &mut impl WriteColor, color_spec: &ColorSpec) -> io::Result<()> {
        const COLOR_WIDTH: usize = 8; // `magenta*` is the widest color output

        out.set_color(color_spec)?;
        out.write_all(if color_spec.bold() { b"b" } else { b"-" })?;
        out.write_all(if color_spec.italic() { b"i" } else { b"-" })?;
        out.write_all(if color_spec.underline() { b"u" } else { b"-" })?;
        out.write_all(if color_spec.dimmed() { b"d" } else { b"-" })?;

        write!(
            out,
            " {fg:>color_width$}/{bg:<color_width$}",
            color_width = COLOR_WIDTH,
            fg = color_spec
                .fg()
                .map_or_else(|| "(none)".to_owned(), |&fg| Self::color_to_string(fg)),
            bg = color_spec
                .bg()
                .map_or_else(|| "(none)".to_owned(), |&bg| Self::color_to_string(bg)),
        )?;
        out.reset()
    }

    fn color_to_string(color: Color) -> String {
        match color {
            Color::Black | Color::Ansi256(0) => "black".to_owned(),
            Color::Red | Color::Ansi256(1) => "red".to_owned(),
            Color::Green | Color::Ansi256(2) => "green".to_owned(),
            Color::Yellow | Color::Ansi256(3) => "yellow".to_owned(),
            Color::Blue | Color::Ansi256(4) => "blue".to_owned(),
            Color::Magenta | Color::Ansi256(5) => "magenta".to_owned(),
            Color::Cyan | Color::Ansi256(6) => "cyan".to_owned(),
            Color::White | Color::Ansi256(7) => "white".to_owned(),

            Color::Ansi256(8) => "black*".to_owned(),
            Color::Ansi256(9) => "red*".to_owned(),
            Color::Ansi256(10) => "green*".to_owned(),
            Color::Ansi256(11) => "yellow*".to_owned(),
            Color::Ansi256(12) => "blue*".to_owned(),
            Color::Ansi256(13) => "magenta*".to_owned(),
            Color::Ansi256(14) => "cyan*".to_owned(),
            Color::Ansi256(15) => "white*".to_owned(),

            Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),

            _ => unreachable!(), // must be transformed during color normalization
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

    fn highlight_spec(self) -> ColorSpec {
        let mut spec = ColorSpec::new();
        match self {
            Self::Main => {
                spec.set_fg(Some(Color::White)).set_bg(Some(Color::Red));
            }
            Self::Aux => {
                spec.set_fg(Some(Color::Black)).set_bg(Some(Color::Yellow));
            }
        }
        spec
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::html::HtmlWriter;
    use termcolor::NoColor;

    #[test]
    fn getting_spans_basics() {
        let spans = ColorSpan::parse("Apr 18 12:54\n\u{1b}[0m\u{1b}[33m.\u{1b}[0m").unwrap();

        assert_eq!(spans.len(), 2);
        assert!(spans[0].color_spec.is_none());
        assert_eq!(spans[0].len, 13);
        assert_eq!(
            spans[1].color_spec,
            *ColorSpec::new().set_fg(Some(Color::Yellow))
        );
        assert_eq!(spans[1].len, 1);
    }

    #[test]
    fn creating_color_diff_basics() {
        let lhs = [ColorSpan {
            len: 5,
            color_spec: ColorSpec::default(),
        }];
        let mut red = ColorSpec::new();
        red.set_fg(Some(Color::Red));
        let rhs = [
            ColorSpan {
                len: 2,
                color_spec: ColorSpec::default(),
            },
            ColorSpan {
                len: 3,
                color_spec: red.clone(),
            },
        ];

        let color_diff = ColorDiff::new(&lhs, &rhs);

        assert_eq!(color_diff.differing_spans.len(), 1);
        let diff_span = &color_diff.differing_spans[0];
        assert_eq!(diff_span.start, 2);
        assert_eq!(diff_span.len, 3);
        assert_eq!(diff_span.lhs_color_spec, ColorSpec::default());
        assert_eq!(diff_span.rhs_color_spec, red);
    }

    #[test]
    fn creating_color_diff_overlapping_spans() {
        let mut red = ColorSpec::new();
        red.set_fg(Some(Color::Red));
        let mut blue = ColorSpec::new();
        blue.set_bg(Some(Color::Blue));

        let lhs = [
            ColorSpan {
                len: 2,
                color_spec: ColorSpec::default(),
            },
            ColorSpan {
                len: 3,
                color_spec: red.clone(),
            },
        ];
        let rhs = [
            ColorSpan {
                len: 1,
                color_spec: ColorSpec::default(),
            },
            ColorSpan {
                len: 2,
                color_spec: red.clone(),
            },
            ColorSpan {
                len: 2,
                color_spec: blue.clone(),
            },
        ];

        let color_diff = ColorDiff::new(&lhs, &rhs);
        assert_eq!(color_diff.differing_spans.len(), 2);
        assert_eq!(color_diff.differing_spans[0].start, 1);
        assert_eq!(color_diff.differing_spans[0].len, 1);
        assert_eq!(
            color_diff.differing_spans[0].lhs_color_spec,
            ColorSpec::default()
        );
        assert_eq!(color_diff.differing_spans[0].rhs_color_spec, red);
        assert_eq!(color_diff.differing_spans[1].start, 3);
        assert_eq!(color_diff.differing_spans[1].len, 2);
        assert_eq!(color_diff.differing_spans[1].lhs_color_spec, red);
        assert_eq!(color_diff.differing_spans[1].rhs_color_spec, blue);
    }

    fn color_spec_to_string(spec: &ColorSpec) -> String {
        let mut buffer = vec![];
        let mut out = NoColor::new(&mut buffer);
        ColorDiff::write_color_spec(&mut out, spec).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn writing_color_spec() {
        let mut spec = ColorSpec::new();
        spec.set_bold(true);
        spec.set_fg(Some(Color::Cyan));
        let spec_string = color_spec_to_string(&spec);
        assert_eq!(spec_string, "b---     cyan/(none)  ");

        spec.set_underline(true);
        spec.set_bg(Some(Color::Ansi256(11)));
        let spec_string = color_spec_to_string(&spec);
        assert_eq!(spec_string, "b-u-     cyan/yellow* ");

        spec.set_italic(true);
        spec.set_bold(false);
        spec.set_fg(Some(Color::Rgb(0xc0, 0xff, 0xee)));
        let spec_string = color_spec_to_string(&spec);
        assert_eq!(spec_string, "-iu-  #c0ffee/yellow* ");
    }

    #[test]
    fn writing_color_diff_table() {
        const EXPECTED_TABLE_LINES: &[&str] = &[
            "Positions      Expected style          Actual style     ",
            "========== ====================== ======================",
            "      0..2 ----   (none)/(none)   b---      red/white   ",
        ];

        let mut red = ColorSpec::new();
        red.set_bold(true)
            .set_fg(Some(Color::Red))
            .set_bg(Some(Color::White));
        let color_diff = ColorDiff {
            differing_spans: vec![DiffColorSpan {
                start: 0,
                len: 2,
                lhs_color_spec: ColorSpec::default(),
                rhs_color_spec: red,
            }],
        };

        let mut buffer = vec![];
        let mut out = NoColor::new(&mut buffer);
        color_diff.write_as_table(&mut out).unwrap();
        let table_string = String::from_utf8(buffer).unwrap();

        for (actual, &expected) in table_string.lines().zip(EXPECTED_TABLE_LINES) {
            assert_eq!(actual, expected);
        }
    }

    fn diff_span(start: usize, len: usize) -> DiffColorSpan {
        DiffColorSpan {
            start,
            len,
            lhs_color_spec: ColorSpec::default(),
            rhs_color_spec: ColorSpec::default(),
        }
    }

    #[test]
    fn highlighting_diff_on_text() {
        let mut green = ColorSpec::default();
        green.set_fg(Some(Color::Green));
        let color_spans = [
            ColorSpan {
                len: 2,
                color_spec: ColorSpec::default(),
            },
            ColorSpan {
                len: 11,
                color_spec: green,
            },
        ];
        let color_diff = ColorDiff {
            differing_spans: vec![
                diff_span(0, 2),
                diff_span(2, 2),
                diff_span(4, 1),
                diff_span(10, 1),
            ],
        };

        let mut buffer = String::new();
        let mut out = HtmlWriter::new(&mut buffer, None);
        color_diff
            .highlight_text(&mut out, "Hello, world!", &color_spans)
            .unwrap();
        assert_eq!(
            buffer,
            "<span class=\"fg1\">&gt; </span>He<span class=\"fg2\">llo, world!</span>\n\
             <span class=\"fg1\">&gt; </span><span class=\"fg7 bg1\">^^</span>\
             <span class=\"fg0 bg3\">!!</span><span class=\"fg7 bg1\">^</span>     \
             <span class=\"fg7 bg1\">^</span>\n"
        );
    }

    #[test]
    fn spans_on_multiple_lines() {
        let mut green = ColorSpec::default();
        green.set_fg(Some(Color::Green));
        let color_spans = [
            ColorSpan {
                len: 9,
                color_spec: green,
            },
            ColorSpan {
                len: 4,
                color_spec: ColorSpec::default(),
            },
        ];

        let color_diff = ColorDiff {
            differing_spans: vec![diff_span(9, 3)],
        };

        let mut buffer = String::new();
        let mut out = HtmlWriter::new(&mut buffer, None);
        color_diff
            .highlight_text(&mut out, "Hello,\nworld!", &color_spans)
            .unwrap();
        assert_eq!(
            buffer,
            "= <span class=\"fg2\">Hello,</span>\n\
             <span class=\"fg1\">&gt; </span><span class=\"fg2\">wo</span>rld!\n\
             <span class=\"fg1\">&gt; </span>  <span class=\"fg7 bg1\">^^^</span>\n"
        );
    }

    #[test]
    fn spans_with_multiple_sequential_line_breaks() {
        let mut green = ColorSpec::default();
        green.set_fg(Some(Color::Green));
        let color_spans = [
            ColorSpan {
                len: 6,
                color_spec: green.clone(),
            },
            ColorSpan {
                len: 4,
                color_spec: ColorSpec::default(),
            },
            ColorSpan {
                len: 4,
                color_spec: green,
            },
        ];

        let color_diff = ColorDiff {
            differing_spans: vec![diff_span(10, 3)],
        };

        let mut buffer = String::new();
        let mut out = HtmlWriter::new(&mut buffer, None);
        color_diff
            .highlight_text(&mut out, "Hello,\n\nworld!", &color_spans)
            .unwrap();

        assert_eq!(
            buffer,
            "= <span class=\"fg2\">Hello,</span>\n\
             = \n\
             <span class=\"fg1\">&gt; </span>wo<span class=\"fg2\">rld!</span>\n\
             <span class=\"fg1\">&gt; </span>  <span class=\"fg7 bg1\">^^^</span>\n"
        );
    }

    fn test_highlight(color_diff: &ColorDiff, text: &str) -> String {
        let color_span = ColorSpan {
            len: text.len(),
            color_spec: ColorSpec::default(),
        };
        let mut buffer = vec![];
        color_diff
            .highlight_text(&mut NoColor::new(&mut buffer), text, &[color_span])
            .unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn plaintext_highlight_simple() {
        let color_diff = ColorDiff {
            differing_spans: vec![
                diff_span(0, 2),
                diff_span(2, 2),
                diff_span(4, 1),
                diff_span(10, 1),
            ],
        };

        let buffer = test_highlight(&color_diff, "Hello, world!");
        let expected_buffer = // (prevents formatter from breaking alignment)
            "> Hello, world!\n\
             > ^^!!^     ^\n";
        assert_eq!(buffer, expected_buffer);
    }

    #[test]
    fn plaintext_highlight_with_multiple_lines() {
        let color_diff = ColorDiff {
            differing_spans: vec![diff_span(4, 12)],
        };

        let buffer = test_highlight(&color_diff, "Hello,\nworld!\nMore text");
        let expected_buffer = // (prevents formatter from breaking alignment)
            "> Hello,\n\
             >     ^^\n\
             > world!\n\
             > ^^^^^^\n\
             > More text\n\
             > ^^\n";
        assert_eq!(buffer, expected_buffer);
    }

    #[test]
    fn plaintext_highlight_with_skipped_lines() {
        let color_diff = ColorDiff {
            differing_spans: vec![diff_span(4, 6), diff_span(26, 2)],
        };

        let buffer = test_highlight(&color_diff, "Hello,\nworld!\nMore\ntext\nhere");
        let expected_buffer = // (prevents formatter from breaking alignment)
            "> Hello,\n\
             >     ^^\n\
             > world!\n\
             > ^^^\n\
             = More\n\
             = text\n\
             > here\n\
             >   ^^\n";
        assert_eq!(buffer, expected_buffer);
    }
}
