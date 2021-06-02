# term-transcript CLI

[![Build Status](https://github.com/slowli/term-transcript/workflows/Rust/badge.svg?branch=master)](https://github.com/slowli/term-transcript/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/term-transcript#license)
![rust 1.45.0+ required](https://img.shields.io/badge/rust-1.45.0+-blue.svg?label=Required%20Rust)

This crate provides command-line interface for [`term-transcript`]. It allows capturing
terminal output to SVG and testing the captured snapshots.

## Usage

- The `capture` subcommand captures output from stdin, renders it to SVG and
  outputs SVG to stdout.
- The `exec` subcommand executes one or more commands in the shell, captures
  their outputs, renders to an SVG image and outputs it to stdout.
- The `test` subcommand allows testing snapshots from the command line.

Launch the CLI with the `--help` option for more details about arguments
for each subcommand.

### Examples

This example creates [a snapshot][snapshot-link]
of [the `rainbow` example][rainbow-example-link] and then tests it.

![Testing rainbow example][test-snapshot-link]

The snapshot itself [is tested][test-link], too! It also shows
that SVG output by the program is editable; in the snapshot, this is used to
highlight command-line args and to change color of comments in the user inputs.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `term-transcript` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions. 

[`term-transcript`]: https://crates.io/crates/term-transcript
[snapshot-link]: https://github.com/slowli/term-transcript/blob/master/examples/rainbow.svg
[rainbow-example-link]: https://github.com/slowli/term-transcript/tree/master/e2e-tests/rainbow
[test-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/test.svg?sanitize=true
[test-link]: https://github.com/slowli/term-transcript/blob/master/cli/tests/e2e.rs
