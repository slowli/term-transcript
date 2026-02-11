//! Basic types.

use core::fmt;

use self::slice::SpansSlice;
pub(crate) use self::spans::StyledSpan;
pub use self::{
    lines::Lines,
    spans::SpanStr,
    str::StyledStr,
    string::{StyledString, StyledStringBuilder},
};
use crate::{
    StyleDiff,
    utils::{Stack, StackStr},
};

mod lines;
mod slice;
mod spans;
mod str;
mod string;

/// Text difference between two strings. ANSI-styled when printed (powered by [`pretty_assertions::Comparison`]).
///
/// # [`Display`](fmt::Display) representation
///
/// You can specify additional padding at the start of compared lines
/// via alignment specifiers. For example, `{:>4}` will insert 4 spaces at the start of each line.
///
/// # Examples
///
/// ```
/// use styled_str::{StyledString, TextDiff};
///
/// let diff = TextDiff::new("Hello, world", "Hello world!");
/// let diff_str = StyledString::from_ansi(&format!("{diff:>4}"))?;
/// assert_eq!(
///     diff_str.text().trim(),
///     "Diff < left / right > :\n\
///      <   Hello, world\n\
///      >   Hello world!"
/// );
/// assert!(!diff_str.as_str().is_plain());
/// # anyhow::Ok(())
/// ```
#[derive(Debug)]
pub struct TextDiff<'a> {
    lhs: &'a str,
    rhs: &'a str,
}

impl<'a> TextDiff<'a> {
    /// Computes difference between two strings.
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

/// Generic difference between two [`StyledStr`]s: either a difference in text, or in styling.
///
/// Produced by the [`StyledStr::diff()`] method.
pub enum Diff<'a> {
    /// There is a difference in text between the compared strings.
    Text(TextDiff<'a>),
    /// String texts match, but there is a difference in ANSI styles.
    Style(StyleDiff<'a>),
}

impl fmt::Display for Diff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(diff) => write!(formatter, "styled strings differ by text\n{diff}"),
            Self::Style(diff) => write!(
                formatter,
                "styled strings differ by style\n{diff}\n{diff:#}"
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

#[cfg(feature = "std")]
impl std::error::Error for Diff<'_> {}

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

    pub const fn as_ref(&'static self) -> StyledStr<'static> {
        StyledStr {
            text: self.text.as_str(),
            spans: SpansSlice::new(self.spans.as_slice()),
        }
    }
}
