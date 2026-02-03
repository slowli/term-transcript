# term-transcript CLI

[![CI](https://github.com/slowli/term-transcript/actions/workflows/ci.yml/badge.svg)](https://github.com/slowli/term-transcript/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/term-transcript#license)
[![The Book](https://img.shields.io/badge/The%20Book-yellow?logo=mdbook)](https://slowli.github.io/term-transcript/)

This crate provides command-line interface for [`term-transcript`]. It allows capturing
terminal output to SVG and testing the captured snapshots.

## Installation

Install with

```shell
cargo install --locked term-transcript-cli
# This will install `term-transcript` executable, which can be checked
# as follows:
term-transcript --help
```

See [the Book](https://slowli.github.io/term-transcript/cli/#installation-options) for more installation options.

## Usage

- The `capture` subcommand captures output from stdin, renders it to SVG and
  outputs SVG to stdout.
- The `exec` subcommand executes one or more commands in the shell, captures
  their outputs, renders to an SVG image and outputs it to stdout.
- The `test` subcommand allows testing snapshots from the command line.
- The `print` subcommand parses an SVG snapshot and outputs it to the command line.

Launch the CLI app with the `--help` option for more details about arguments
for each subcommand. See [the Book](https://slowli.github.io/term-transcript/examples/) for more detailed overview of command-line args and options.

### Examples

This example creates a snapshot of [the `rainbow` script][rainbow-script-link] and then tests it.

![Testing rainbow example][test-snapshot-link]

The snapshot itself [is tested][test-link], too! It also shows
that SVG output by the program is editable; in the snapshot, this is used to
highlight command-line args and to change color of comments in the user inputs.

The `test` command can compare colors as well:

![Testing color match][test-color-snapshot-link]

Another snapshot created by capturing help output from a pseudo-terminal
(the `--pty` flag):

![Output of `test-transcript --help`][help-snapshot-link]

Using PTY enables coloring output by default and formatting dependent
on the terminal size.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `term-transcript` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions. 

[`term-transcript`]: https://crates.io/crates/term-transcript
[fmt-subscriber]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html
[rainbow-script-link]: ../e2e-tests/rainbow/bin/rainbow
[test-snapshot-link]: tests/snapshots/test.svg
[test-color-snapshot-link]: tests/snapshots/test-fail.svg
[test-link]: tests/e2e.rs
[help-snapshot-link]: tests/snapshots/help.svg
[`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
