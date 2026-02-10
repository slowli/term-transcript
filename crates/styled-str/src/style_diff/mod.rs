//! Comparing `Styled` instances by styling.

use core::{
    cmp::{self, Ordering},
    fmt,
    iter::{self, Peekable},
    mem,
    num::NonZeroUsize,
};

use anstyle::{AnsiColor, Color, Effects, Style};
use unicode_width::UnicodeWidthStr;

use crate::{
    StyledStr,
    alloc::{String, Vec, format},
    rich_parser::RichStyle,
    types::StyledSpan,
};

#[cfg(test)]
mod tests;

impl StyledSpan {
    fn shrink_len(&mut self, sub: usize) {
        self.len = self
            .len
            .get()
            .checked_sub(sub)
            .and_then(NonZeroUsize::new)
            .expect("length underflow");
    }

    /// Writes a single plaintext `line` to `out` using styles from `spans_iter`.
    fn write_line<I>(
        formatter: &mut fmt::Formatter<'_>,
        spans_iter: &mut Peekable<I>,
        line_start: usize,
        line: &str,
    ) -> fmt::Result
    where
        I: Iterator<Item = Self>,
    {
        let mut pos = 0;
        while pos < line.len() {
            let span = spans_iter.peek().expect("spans ended before lines");
            let span_len = span.len.get() - line_start.saturating_sub(span.start);

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
    len: NonZeroUsize,
    lhs_style: Style,
    rhs_style: Style,
}

impl DiffStyleSpan {
    /// Would this style be visible on whitespace (e.g., ' ' or '\t')?
    fn affects_whitespace(style: &Style) -> bool {
        let effects = style.get_effects();
        if effects.contains(Effects::UNDERLINE)
            || effects.contains(Effects::STRIKETHROUGH)
            || effects.contains(Effects::INVERT)
        {
            return true;
        }
        // We've handled the case with inverted colors above, so we check the background color specifically
        style.get_bg_color().is_some()
    }

    /// Trims whitespace at the start and end of the string; we don't care about diffs there.
    fn new(diff_text: &str, start: usize, lhs_style: Style, rhs_style: Style) -> Option<Self> {
        debug_assert!(!diff_text.is_empty());

        let affects_whitespace =
            Self::affects_whitespace(&lhs_style) || Self::affects_whitespace(&rhs_style);
        let can_trim = |ch: char| {
            if affects_whitespace {
                // Newline chars are not affected by any styles
                ch == '\n' || ch == '\r'
            } else {
                ch.is_whitespace()
            }
        };

        let first_pos = diff_text
            .char_indices()
            .find_map(|(i, ch)| (!can_trim(ch)).then_some(i))?;
        let last_pos = diff_text
            .char_indices()
            .rev()
            .find_map(|(i, ch)| (!can_trim(ch)).then_some(i + ch.len_utf8()))?;
        debug_assert!(last_pos > first_pos);

        Some(Self {
            start: start + first_pos,
            len: NonZeroUsize::new(last_pos - first_pos)?,
            lhs_style,
            rhs_style,
        })
    }
}

const STYLE_WIDTH: usize = 25;

/// Difference in styles between two [styled strings](StyledStr) that can be output in detailed
/// human-readable format via [`Display`](fmt::Display).
///
/// The `Display` implementation supports two output formats:
///
/// - With the default / non-alternate format (e.g., `{}`), the diff will be output as the LHS line-by-line,
///   highlighting differences in styles on each differing line. That is, this format is similar
///   to [`pretty_assertions::Comparison`].
/// - With the alternate format (`{:#}`), the diff will be output as a table of all differing spans.
///
/// # Examples
///
/// ```
/// use styled_str::{styled, StyleDiff, StyledString};
///
/// let lhs = styled!("[[red on white]]Hello,[[/]] [[bold green]]world!");
/// let rhs = styled!("[[red on white!]]Hello,[[/]] [[bold green]]world[[/]]!");
/// let diff = StyleDiff::new(lhs, rhs);
/// assert!(!diff.is_empty());
///
/// let diff_str = StyledString::from_ansi(&format!("{diff}"))?;
/// assert_eq!(
///     diff_str.text(),
///     "> Hello, world!\n\
///      > ^^^^^^      ^\n"
/// );
///
/// let diff_str = StyledString::from_ansi(&format!("{diff:#}"))?;
/// let expected =
///     "|Positions         Left style                Right style       |
///      |========== ========================= =========================|
///      |      0..6       red on white              red on white!      |
///      |    12..13        bold green                  (none)          |";
/// let expected: String = expected
///     .lines()
///     .flat_map(|line| [line.trim().trim_matches('|'), "\n"])
///     .collect();
/// assert_eq!(diff_str.text(), expected);
/// # anyhow::Ok(())
/// ```
#[derive(Debug)]
pub struct StyleDiff<'a> {
    lhs: StyledStr<'a>,
    differing_spans: Vec<DiffStyleSpan>,
}

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
    /// Computes style difference between two styled strings.
    ///
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
            lhs,
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
                let diff_text = &this.lhs.text[pos..pos + common_len.get()];
                this.differing_spans.extend(DiffStyleSpan::new(
                    diff_text,
                    pos,
                    lhs_span.style,
                    rhs_span.style,
                ));
            }

            pos += common_len.get();

            match lhs_span.len.cmp(&rhs_span.len) {
                Ordering::Less => {
                    rhs_span.shrink_len(lhs_span.len.get());
                    lhs_span = lhs_iter.next().unwrap();
                    // ^ `unwrap()` here and below are safe; we've checked that
                    // `lhs` and `rhs` contain same total span coverage.
                }
                Ordering::Greater => {
                    lhs_span.shrink_len(rhs_span.len.get());
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

    /// Checks whether this difference is empty.
    pub fn is_empty(&self) -> bool {
        self.differing_spans.is_empty()
    }

    /// Highlights this diff on the specified `text` which has styling set with `color_spans`.
    fn highlight_text(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SIDELINE_HL: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));

        let highlights = HighlightedSpan::new(&self.differing_spans);
        let mut highlights = highlights.iter().copied().peekable();
        let mut line_start = 0;

        let mut color_spans = self.lhs.spans.iter().copied().peekable();

        for line in self.lhs.text.split('\n') {
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
            let span_end = cmp::min(span.start + span.len.get() - line_offset, line_len);

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
            if span.start + span.len.get() <= line_offset + line_len {
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
            let lhs_style = &differing_span.lhs_style;
            let mut lhs_lines = Self::write_style(lhs_style);
            let rhs_style = &differing_span.rhs_style;
            let mut rhs_lines = Self::write_style(rhs_style);
            if lhs_lines.len() < rhs_lines.len() {
                lhs_lines.resize_with(rhs_lines.len(), String::new);
            } else {
                rhs_lines.resize_with(lhs_lines.len(), String::new);
            }

            for (i, (lhs_line, rhs_line)) in lhs_lines.into_iter().zip(rhs_lines).enumerate() {
                if i == 0 {
                    let start = differing_span.start;
                    let end = start + differing_span.len.get();
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

        let mut lines = Vec::new();
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
    len: NonZeroUsize,
    kind: SpanHighlightKind,
}

impl HighlightedSpan {
    fn new(differing_spans: &[DiffStyleSpan]) -> Vec<Self> {
        let mut sequential_span_count = 1;
        let span_highlights = differing_spans.windows(2).map(|window| match window {
            [prev, next] => {
                if prev.start + prev.len.get() == next.start {
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
