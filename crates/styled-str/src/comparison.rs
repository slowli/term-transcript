//! Comparison highlighting powered by the `similar` crate.
//!
//! Produces similar formatting to the `pretty_assertions` crate (the crate itself is not used because it would make `styled-str`
//! `std`-dependent).

use core::fmt;

use similar::{ChangeTag, InlineChangeMode, InlineChangeOptions, TextDiff};

const RED_LIGHT: &str = "\u{1b}[31m";
const GREEN_LIGHT: &str = "\u{1b}[32m";
const RED_HEAVY: &str = "\u{1b}[1;48;5;52;31m";
const GREEN_HEAVY: &str = "\u{1b}[1;48;5;22;32m";
const BOLD: &str = "\u{1b}[1m";
const ITALIC: &str = "\u{1b}[3m";
const RESET: &str = "\u{1b}[0m";

#[derive(Debug)]
struct NormalizedSegments<'a> {
    inner: &'a [(bool, &'a str)],
    last_replacement: Option<(bool, &'a str)>,
}

impl<'a> NormalizedSegments<'a> {
    fn new(mut src: &'a [(bool, &'a str)]) -> Self {
        if src.last().is_some_and(|&(_, s)| s == "\n") {
            src = &src[..src.len() - 1];
        }

        let mut last_replacement = if let Some(&(hl, last)) = src.last() {
            last.strip_suffix('\n').map(|trimmed| {
                src = &src[..src.len() - 1];
                (hl, trimmed)
            })
        } else {
            None
        };

        if let &[(true, segment)] = src {
            // Remove highlight for single-segment lines
            if let Some((hl, _)) = &mut last_replacement {
                *hl = false;
            } else {
                src = &src[..src.len() - 1];
                last_replacement = Some((false, segment));
            }
        }

        Self {
            inner: src,
            last_replacement,
        }
    }

    fn into_iter(self) -> impl Iterator<Item = (bool, &'a str)> {
        self.inner.iter().copied().chain(self.last_replacement)
    }
}

/// Writes a static diff header to the provided `writer`.
pub(crate) fn write_diff_header(writer: &mut impl fmt::Write) -> fmt::Result {
    writeln!(
        writer,
        "{BOLD}Diff{RESET} {RED_LIGHT}< left{RESET} / {GREEN_LIGHT}right >{RESET} :"
    )
}

/// Writes a `pretty_assertions`-inspired text diff styled with ANSI escape sequences.
/// The diff is two-level (lines + chars within lines).
pub(crate) fn highlight_diff(
    writer: &mut impl fmt::Write,
    left: &str,
    right: &str,
    padding: usize,
) -> fmt::Result {
    compare(left, right, |tag, segments| {
        let segments = NormalizedSegments::new(segments);
        match tag {
            ChangeTag::Equal => {
                // Just output the segments without highlights
                write!(writer, "{:>full_padding$}", "", full_padding = padding + 1)?;
                for (_, segment) in segments.into_iter() {
                    write!(writer, "{segment}")?;
                }
            }
            ChangeTag::Delete => {
                write!(writer, "{RED_LIGHT}<{RESET}{:>padding$}", "")?;
                for (hl, segment) in segments.into_iter() {
                    let style = if hl { RED_HEAVY } else { RED_LIGHT };
                    write!(writer, "{style}{segment}{RESET}")?;
                }
            }
            ChangeTag::Insert => {
                write!(writer, "{GREEN_LIGHT}>{RESET}{:>padding$}", "")?;
                for (hl, segment) in segments.into_iter() {
                    let style = if hl { GREEN_HEAVY } else { GREEN_LIGHT };
                    write!(writer, "{style}{segment}{RESET}")?;
                }
            }
        }
        writeln!(writer)
    })?;

    // Since we trim newlines, we need to process the trailing newline separately. Note that we do handle
    // multiple trailing newlines correctly.
    let left_nl = left.ends_with('\n');
    let right_nl = right.ends_with('\n');
    match (left_nl, right_nl) {
        (true, false) => {
            writeln!(
                writer,
                "{RED_LIGHT}<{:>padding$}{ITALIC}(trailing newline removed){RESET}",
                ""
            )
        }
        (false, true) => {
            writeln!(
                writer,
                "{GREEN_LIGHT}>{:>padding$}{ITALIC}(trailing newline added){RESET}",
                ""
            )
        }
        _ => Ok(()),
    }
}

fn compare(
    left: &str,
    right: &str,
    mut hook: impl FnMut(ChangeTag, &[(bool, &str)]) -> fmt::Result,
) -> fmt::Result {
    let line_diff = TextDiff::from_lines(left, right);
    let mut options = InlineChangeOptions::new();
    options.mode(InlineChangeMode::Chars);

    for change in line_diff.iter_all_inline_changes_with_options(options) {
        hook(change.tag(), change.values())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::{string::String, vec};

    use super::*;
    use crate::{StyledString, styled};

    #[test]
    fn simple_diffing() {
        let left = "Old\nTest\n:str value";
        let right = "Test\nstring value\nMore";

        let mut expected = vec![
            (ChangeTag::Delete, &[(false, "Old\n")] as &[_]),
            (ChangeTag::Equal, &[(false, "Test\n")]),
            (
                ChangeTag::Delete,
                &[(true, ":"), (false, "str"), (false, " value")],
            ),
            (
                ChangeTag::Insert,
                &[
                    (false, "str"),
                    (true, "ing"),
                    (false, " value"),
                    (false, "\n"),
                ],
            ),
            (ChangeTag::Insert, &[(true, "More")]),
        ];
        compare(left, right, |tag, line| {
            assert_eq!((tag, line), expected.remove(0));
            Ok(())
        })
        .unwrap();
        assert_eq!(expected, []);
    }

    #[test]
    fn diff_highlighting() {
        let left = "Test\nOld\n:str value!";
        let right = "Test\nstring value?\nMore\n";

        let mut buffer = String::new();
        highlight_diff(&mut buffer, left, right, 2).unwrap();

        let buffer = StyledString::from_ansi(&buffer).unwrap();
        let expected = styled!(
            "   Test
[[red]]<[[/]]  [[red]]Old[[/]]
[[red]]<[[/]]  [[bold red on #5f0000]]:[[red]]str value[[bold red on #5f0000]]![[/]]
[[green]]>[[/]]  [[green]]str[[bold green on #005f00]]ing[[green]] value[[bold green on #005f00]]?[[/]]
[[green]]>[[/]]  [[green]]More[[/]]
[[green]]>  [[italic green]](trailing newline added)[[/]]
"
        );
        assert_eq!(buffer, expected);
    }

    #[test]
    fn more_diff_highlighting() {
        let left = "# Test?\nText\n";
        let right = "# Test title\n- Text\n- More text\n";

        let mut buffer = String::new();
        highlight_diff(&mut buffer, left, right, 2).unwrap();
        let buffer = StyledString::from_ansi(&buffer).unwrap();
        let expected = styled!(
            "\
[[red]]<[[/]]  [[red]]# Test[[bold red on #5f0000]]?[[/]]
[[red]]<[[/]]  [[red]]Text[[/]]
[[green]]>[[/]]  [[green]]# Test[[bold green on #005f00]] title[[/]]
[[green]]>[[/]]  [[bold green on #005f00]]- [[green]]Text[[/]]
[[green]]>[[/]]  [[green]]- More text[[/]]
"
        );
        assert_eq!(buffer, expected);
    }
}
