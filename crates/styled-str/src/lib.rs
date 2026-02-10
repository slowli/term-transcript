//! Tools for parsing and managing ANSI-styled strings.
//!
//! This library allows to:
//!
//! - [Parse ANSI-styled strings](#parsing-ansi-escapes).
//! - Create styled strings from [human-readable format](#rich-format), including in compile time.
//! - Compare styled strings with [rich diff info](#comparing-styled-strings).
//! - Manipulate styled strings, e.g. split them into lines, split off parts etc.
//!
//! One of guiding use cases for the library is hassle-free snapshot testing for styled strings,
//! without the need to compare literal strings with ANSI escapes (which is brittle and not human-readable),
//! and outputting more informative diff info than a simple `assert_eq!` would.
//!
//! For the example of real-world usage, see the [`term-transcript`](https://docs.rs/term-transcript/) crate.
//!
//! # `Styled` type
//!
//! The core type exposed by this crate is [`Styled`]. This is a generic container for text + styling
//! signaled via [ANSI escape codes].
//! Its two main instantiations are borrowed [`StyledStr`] (analog to `&str`) and owned
//! [`StyledString`] (analog to `String`). Styling is represented as a sequence of [`StyledSpan`]s
//! that cover the text in its entirety. Styles reuse the model from [`anstyle`]; i.e., a style
//! is just a [`Style`](anstyle::Style).
//!
//! # Rich syntax
//!
//! One way to create `Styled` strings is parsing [`rich`]-inspired syntax, either in compile time
//! via the [`styled!`] macro, or in runtime via [`FromStr`](core::str::FromStr).
//! Conversely, a `Styled` string can be converted to the rich format via its [`Display`](core::fmt::Display)
//! implementation.
//!
//! The format is as follows:
//!
//! - Styling directives are enclosed in double brackets: `[[` + `]]`.
//! - A directive is a list of *tokens* separated by whitespace and/or commas `,` or semicolons `;`.
//! - A token represents an effect, e.g. `bold` or `underline`, a foreground [color](#color-tokens) (e.g., `red`
//!   or `#fed`), or a background color (`on` + a color token, e.g. `on blue`).
//! - By default, a directive completely overrides the previously applied directive. Hence, there is no need
//!   for special closing directives.
//! - A directive can be made to inherit from the previously applied style by preceding all tokens with a `*`.
//!   This also allows to *subtract* effects by specifying them with a `-` or `!` in front, like `-bold` or `!italic`.
//!   Similarly, `-color` / `!color` (or `-fg` / `!fg`) switches off the foreground color, and
//!   `-on`, `!on`, `-bg`, or `!bg` switches off the background color.
//! - A directive may be empty: `[[]]`. As a special case, `[[/]]` (i.e., a single `/` token) is equivalent
//!   to an empty directive.
//!
//! ## Effects
//!
//! The following effects are supported:
//!
//! | Effect | Aliases |
//! |:-------|:--------|
//! | `bold` | `b` |
//! | `italic` | `it`, `i` |
//! | `underline`| `ul`, `u` |
//! | `strikethrough` | `strike`, `s` |
//! | `dimmed` | `dim` |
//! | `invert` | `inverted`, `inv` |
//! | `blink` | |
//! | `concealed` | `conceal`, `hide`, `hidden` |
//!
//! ## Color tokens
//!
//! A color may be represented as follows:
//!
//! - One of the 8 base terminal colors (`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`)
//! - One of the 8 bright terminal colors signaled via `!` suffix or `bright-` prefix (e.g., `blue!` or `bright-blue`).
//! - One of [256 indexed ANSI colors](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit) specified
//!   as `color$idx` or `color($idx)`, e.g. `color23` or `color(254)`.
//! - A 24-bit RGB color written in CSS-like hex format, e.g. `#fa4` or `#c0ffee`.
//!
//! ## Syntax examples
//!
//! ```
//! use styled_str::{styled, StyledStr};
//!
//! const STYLED: StyledStr = styled!("[[green! on white, bold]]Hello,[[/]] world[[it]]!");
//! assert_eq!(STYLED.text(), "Hello, world!");
//! assert_eq!(STYLED.spans().len(), 3);
//! ```
//!
//! Here, the first style applies to `Hello,`, then it is completely reset for ` world`,
//! and finally, the italic effect (and only it) is applied to `!`.
//!
//! ```
//! # use styled_str::{styled, StyledStr};
//! const STYLED: StyledStr = styled!("[[bold green!]]Hello[[* -bold]], world[[* invert]]!");
//! ```
//!
//! Here, there are again 3 styled spans, but the second and third ones inherit the preceding style.
//! The second span removes the bold effect, and the third one inverts foreground and background colors.
//!
//! # Other string functionality
//!
//! ## Parsing ANSI escapes
//!
//! [`StyledString::from_ansi()`] and [`StyledString::from_ansi_bytes()`] allow parsing a styled string
//! from ANSI escapes (e.g., captured from a terminal).
//!
//! ```
//! # use styled_str::{styled, AnsiError, StyledString};
//! let str = StyledString::from_ansi(
//!     "\u{1b}[32mHello,\u{1b}[m world\u{1b}[1m!\u{1b}[m",
//! )?;
//! assert_eq!(str.text(), "Hello, world!");
//! assert_eq!(str.to_string(), "[[green]]Hello,[[/]] world[[bold]]!");
//! # anyhow::Ok(())
//! ```
//!
//! ## Comparing styled strings
//!
//! [`Styled::diff()`] allows comparing two styled strings both in terms of text and styles.
//! [`TextDiff`] and [`StyleDiff`] provide more fine-grained control over comparison logic.
//! These types can be [`Display`](core::fmt::Display)ed / [`Debug`](core::fmt::Debug)ged
//! in order to provide rich human-readable info about differences (e.g., in the test code).
//!
//! ```
//! # use styled_str::{styled, Diff};
//! # use assert_matches::assert_matches;
//! let left = styled!("Hello, [[bold dim white on #fa4]]world!");
//! let right = styled!("Hello, [[bold]]world[[/]]!");
//! let diff = left.diff(&right).unwrap_err();
//! assert_matches!(&diff, Diff::Style(_));
//!
//! // Will output detailed human-readable info about the diff.
//! println!("{diff}");
//! ```
//!
//! # Limitations
//!
//! - ANSI escape sequences other than [SGR] ones are either dropped (in case of [CSI] sequences),
//!   or lead to [an error](AnsiError).
//!
//! # Alternatives and similar tools
//!
//! - This crate builds on the [`anstyle`] library, using its styling data model. `anstyle` together
//!   with [`anstream`](https://docs.rs/anstream/) provides tools to create / output ANSI-styled strings in runtime.
//!   It doesn't cover creating strings in compile time, parsing ANSI-styled strings, or comparing styled strings.
//! - [`color_print`](https://docs.rs/color-print/) provides proc macros to create / output ANSI-styled strings
//!   using `rich`-like syntax.
//! - [`parse-style`](https://docs.rs/parse-style/) allows parsing `rich`-like style specs.
//!
//! # Crate features
//!
//! ## `std`
//!
//! *(On by default)*
//!
//! Enables std-specific functionality, such as [`Error`](std::error::Error) trait implementations.
//!
//! [ANSI escape codes]: https://en.wikipedia.org/wiki/ANSI_escape_code
//! [`rich`]: https://rich.readthedocs.io/en/stable/index.html
//! [SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
//! [CSI]: https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences

// Documentation settings
#![doc(html_root_url = "https://docs.rs/styled-str/0.5.0-beta.1")]
// Conditional compilation
#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::{
    ansi_parser::AnsiError,
    errors::{HexColorError, ParseError, ParseErrorKind},
    rich_parser::{RichStyle, parse_hex_color, rgb_color_to_hex},
    style_diff::StyleDiff,
    types::{
        AsSpansSlice, Diff, PopChar, SpansSlice, SpansVec, StackStyled, Styled, StyledStr,
        StyledString, StyledStringBuilder, TextDiff,
    },
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

mod alloc {
    #[cfg(not(feature = "std"))]
    extern crate alloc as std;

    pub(crate) use std::{borrow::Cow, format, string::String, vec::Vec};
}

/// Parses [rich syntax](crate#rich-syntax) into a [`StyledStr`] in compile time.
///
/// # Examples
///
/// ```
/// use styled_str::{styled, StyledStr};
///
/// const STYLED: StyledStr = styled!(
///     "[[bold red on white]]ERROR:[[/]] [[it]]Something[[/]] \
///      [[strike]]bad[[/]] happened"
/// );
/// assert_eq!(STYLED.text(), "ERROR: Something bad happened");
/// assert_eq!(STYLED.spans()[0].len.get(), "ERROR:".len());
/// ```
#[macro_export]
macro_rules! styled {
    ($raw:expr) => {{
        const __CAPACITIES: (usize, usize) = $crate::StyledStr::capacities($raw);
        const { $crate::StackStyled::<{ __CAPACITIES.0 }, { __CAPACITIES.1 }>::new($raw) }.as_ref()
    }};
}

#[cfg(doctest)]
doc_comment::doctest!("../README.md");
