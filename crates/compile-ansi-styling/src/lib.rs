//! Compile-time ANSI styling.

#![allow(missing_docs)] // FIXME

pub use crate::{
    errors::{ParseError, ParseErrorKind},
    types::{DynStyled, StackStyled, Styled, StyledSpan},
};

#[macro_use]
mod utils;
mod errors;
mod rich_parser;
#[cfg(test)]
mod tests;
mod types;

#[macro_export]
macro_rules! styled {
    ($raw:expr) => {{
        const __CAPACITIES: (usize, usize) = $crate::Styled::capacities($raw);
        $crate::StackStyled::<{ __CAPACITIES.0 }, { __CAPACITIES.1 }>::new($raw).as_ref()
    }};
}
