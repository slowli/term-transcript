//! Terminal ANSI styling tools.

#![allow(missing_docs)] // FIXME

pub use crate::{
    ansi_parser::AnsiError,
    errors::{HexColorError, ParseError, ParseErrorKind},
    rich_parser::{parse_hex_color, rgb_color_to_hex},
    style_diff::StyleDiff,
    types::{Diff, StackStyled, Styled, StyledSpan, StyledStr, StyledString, TextDiff},
};

#[macro_use]
mod utils;
mod ansi_parser;
mod errors;
mod rich_parser;
mod style_diff;
#[cfg(test)]
mod tests;
mod types;

#[macro_export]
macro_rules! styled {
    ($raw:expr) => {{
        const __CAPACITIES: (usize, usize) = $crate::StyledStr::capacities($raw);
        const { $crate::StackStyled::<{ __CAPACITIES.0 }, { __CAPACITIES.1 }>::new($raw) }.as_ref()
    }};
}
