//! Basic types.

use core::{fmt, ops};

use anstyle::Style;

use crate::{
    AnsiError, StyleDiff,
    ansi_parser::AnsiParser,
    rich_parser::{EscapedText, RichStyle},
    utils::{Stack, StackStr},
};

/// Continuous span of styled text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyledSpan {
    /// Style applied to the text.
    pub style: Style,
    /// Length of text in bytes.
    pub len: usize,
}

/// ANSI-styled text.
///
/// FIXME: ways to create
// FIXME: rename (StyledStr / StyledString); as_ref()
#[derive(Debug, Clone, Copy, Default)]
pub struct Styled<T = &'static str, S = &'static [StyledSpan]> {
    pub(crate) text: T,
    pub(crate) spans: S,
}

/// Dynamic (i.e., non-compile time) variation of [`Styled`].
pub type DynStyled = Styled<String, Vec<StyledSpan>>;

impl DynStyled {
    /// # Errors
    ///
    /// Returns an error if the input is not a valid ANSI escaped string.
    pub fn from_ansi(ansi_str: &str) -> Result<Self, AnsiError> {
        AnsiParser::parse(ansi_str.as_bytes())
    }

    /// # Errors
    ///
    /// Returns an error if the input is not a valid ANSI escaped string.
    pub fn from_ansi_bytes(ansi_bytes: &[u8]) -> Result<Self, AnsiError> {
        AnsiParser::parse(ansi_bytes)
    }

    /// Unites sequential spans with the same color spec.
    pub(crate) fn shrink(self) -> Self {
        let mut shrunk_spans = Vec::<StyledSpan>::with_capacity(self.spans.len());
        for span in self.spans {
            if let Some(last_span) = shrunk_spans.last_mut() {
                if last_span.style == span.style {
                    last_span.len += span.len;
                } else {
                    shrunk_spans.push(span);
                }
            } else {
                shrunk_spans.push(span);
            }
        }

        Self {
            text: self.text,
            spans: shrunk_spans,
        }
    }
}

impl<T, S> Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: ops::Deref<Target = [StyledSpan]>,
{
    /// Returns the unstyled text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the style spans.
    pub fn spans(&self) -> &[StyledSpan] {
        &self.spans
    }

    /// Diffs this against the `other` styled string.
    ///
    /// # Errors
    ///
    /// Returns an error if the styled strings differ either in text, or in applied styles.
    pub fn diff<'s, Tr, Sr>(&'s self, other: &'s Styled<Tr, Sr>) -> Result<(), Diff<'s>>
    where
        Tr: ops::Deref<Target = str>,
        Sr: ops::Deref<Target = [StyledSpan]>,
    {
        let this_text = self.text();
        let other_text = other.text();
        if this_text == other_text {
            let this_spans = self.spans();
            let other_spans = other.spans();
            let style_diff = StyleDiff::new(this_text, this_spans, other_spans);
            if style_diff.is_empty() {
                Ok(())
            } else {
                Err(Diff::Style(style_diff))
            }
        } else {
            Err(Diff::Text(TextDiff {
                lhs: this_text,
                rhs: other_text,
            }))
        }
    }

    /// Returns a string with embedded ANSI escapes.
    pub fn ansi(&self) -> impl fmt::Display + '_ {
        Ansi {
            text: &self.text,
            spans: &self.spans,
        }
    }
}

/// Outputs a string with rich syntax.
impl<T, S> fmt::Display for Styled<T, S>
where
    T: ops::Deref<Target = str>,
    S: ops::Deref<Target = [StyledSpan]>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pos = 0;
        for (i, span) in self.spans.iter().enumerate() {
            let text = &self.text[pos..pos + span.len];
            if i == 0 && span.style.is_plain() {
                // Special case: do not output an extra `[[]]` at the string start.
                write!(formatter, "{}", EscapedText(text))?;
            } else {
                write!(
                    formatter,
                    "[[{style}]]{text}",
                    style = RichStyle(&span.style),
                    text = EscapedText(text)
                )?;
            }
            pos += span.len;
        }
        Ok(())
    }
}

struct Ansi<'a> {
    text: &'a str,
    spans: &'a [StyledSpan],
}

impl fmt::Display for Ansi<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pos = 0;
        for span in self.spans {
            write!(
                formatter,
                "{style}{text}{style:#}",
                style = span.style,
                text = &self.text[pos..pos + span.len]
            )?;
            pos += span.len;
        }
        Ok(())
    }
}

impl<Tl, Sl, Tr, Sr> PartialEq<Styled<Tr, Sr>> for Styled<Tl, Sl>
where
    Tl: ops::Deref<Target = str>,
    Sl: ops::Deref<Target = [StyledSpan]>,
    Tr: ops::Deref<Target = str>,
    Sr: ops::Deref<Target = [StyledSpan]>,
{
    fn eq(&self, other: &Styled<Tr, Sr>) -> bool {
        *self.text == *other.text && *self.spans == *other.spans
    }
}

#[derive(Debug)]
pub struct TextDiff<'a> {
    lhs: &'a str,
    rhs: &'a str,
}

impl<'a> TextDiff<'a> {
    pub const fn new(lhs: &'a str, rhs: &'a str) -> Self {
        Self { lhs, rhs }
    }
}

impl fmt::Display for TextDiff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use pretty_assertions::Comparison;

        // Since `Comparison` uses `fmt::Debug`, we define this simple wrapper
        // to switch to `fmt::Display`.
        struct DebugStr<'a> {
            s: &'a str,
            padding: usize,
        }

        impl<'a> DebugStr<'a> {
            fn new(s: &'a str, padding: usize) -> Self {
                Self { s, padding }
            }
        }

        impl fmt::Debug for DebugStr<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.padding == 0 {
                    formatter.write_str(self.s)
                } else {
                    for line in self.s.lines() {
                        writeln!(formatter, "{:>padding$}{line}", "", padding = self.padding)?;
                    }
                    Ok(())
                }
            }
        }

        let padding = if matches!(formatter.align(), Some(fmt::Alignment::Right) | None) {
            formatter.width().map_or(0, |width| width.saturating_sub(1))
        } else {
            0
        };

        write!(
            formatter,
            "{}",
            Comparison::new(
                &DebugStr::new(self.lhs, padding),
                &DebugStr::new(self.rhs, padding)
            )
        )
    }
}

pub enum Diff<'a> {
    Text(TextDiff<'a>),
    Style(StyleDiff<'a>),
}

impl fmt::Display for Diff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(diff) => write!(formatter, "Styled strings differ by text\n{diff}"),
            Self::Style(diff) => write!(
                formatter,
                "Styled strings differ by style\n{diff}\n{diff:#}"
            ),
        }
    }
}

// Delegates to `Display` to get better panic messages on `.diff(_).unwrap()`.
impl fmt::Debug for Diff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// Stack-allocated version of [`Styled`] for use in compile-time parsing of rich styling strings.
#[doc(hidden)]
#[derive(Debug)]
pub struct StackStyled<const TEXT_CAP: usize, const SPAN_CAP: usize> {
    pub(crate) text: StackStr<TEXT_CAP>,
    pub(crate) spans: Stack<StyledSpan, SPAN_CAP>,
}

impl<const TEXT_CAP: usize, const SPAN_CAP: usize> StackStyled<TEXT_CAP, SPAN_CAP> {
    /// Instantiates a new instance from a `rich`-flavored string.
    ///
    /// # Panics
    ///
    /// Panics if the rich syntax is invalid.
    #[track_caller]
    pub const fn new(raw: &str) -> Self {
        match Self::parse(raw) {
            Ok(styled) => styled,
            Err(err) => err.compile_panic(raw),
        }
    }

    pub const fn as_ref(&'static self) -> Styled {
        Styled {
            text: self.text.as_str(),
            spans: self.spans.as_slice(),
        }
    }
}
