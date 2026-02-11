# Parsing and Managing ANSI-styled Strings

[![CI](https://github.com/slowli/term-transcript/actions/workflows/ci.yml/badge.svg)](https://github.com/slowli/term-transcript/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/term-transcript#license)
![rust 1.86+ required](https://img.shields.io/badge/rust-1.86+-blue.svg?label=Required%20Rust)

**Documentation:**
[![crate docs (master)](https://img.shields.io/badge/master-yellow.svg?label=docs)](https://slowli.github.io/term-transcript/crates/styled_str/)

This library allows to:

- Parse ANSI-styled strings.
- Create styled strings from a human-readable format inspired by [`rich`], including in compile time.
- Compare styled strings with rich diff info.
- Manipulate styled strings, e.g. split them into lines, split off parts etc.

One of guiding use cases for the library is hassle-free snapshot testing for styled strings,
without the need to compare literal strings with ANSI escapes (which is brittle and not human-readable),
and outputting more informative diff info than a simple `assert_eq!` would.

For the example of real-world usage, see the [`term-transcript`](https://crates.io/crates/term-transcript/) crate.

## Usage

Add this to your `Crate.toml`:

```toml
[dependencies]
styled-str = "0.5.0-beta.1"
```

Basic usage:

```rust
use styled_str::{styled, StyledStr};

const STYLED: StyledStr = styled!(
    "[[bold white! on green!]]INFO:[[/]] [[dim it]]12:00:01[[/]] \
     [[ul #fb4]]Something[[/]] happened"
);

// Get the unstyled text behind the string
assert_eq!(STYLED.text(), "INFO: 12:00:01 Something happened");
// Get the style spans info
assert_eq!(STYLED.spans().len(), 6);

// Print the string with embedded ANSI sequences
println!("{}", STYLED.ansi());
```

See the crate docs for more examples of usage.

## Limitations

- ANSI escape sequences other than [SGR] ones are either dropped (in case of [CSI] sequences),
  or lead to an error.

## Alternatives and similar tools

- This crate builds on the [`anstyle`](https://crates.io/crates/anstyle/) library, using its styling data model. `anstyle` together
  with [`anstream`](https://crates.io/crates/anstream/) provides tools to create / output ANSI-styled strings in runtime.
  It doesn't cover creating strings in compile time, parsing ANSI-styled strings, or comparing styled strings.
- [`color_print`](https://crates.io/crates/color-print/) provides proc macros to create / output ANSI-styled strings
  using `rich`-like syntax.
- [`parse-style`](https://crates.io/crates/parse-style/) allows parsing `rich`-like style specs.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `term-transcript` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[`rich`]: https://rich.readthedocs.io/en/stable/index.html
[SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
[CSI]: https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences
