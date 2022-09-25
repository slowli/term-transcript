# term-transcript CLI

[![Build Status](https://github.com/slowli/term-transcript/workflows/Rust/badge.svg?branch=master)](https://github.com/slowli/term-transcript/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/term-transcript#license)
![rust 1.61+ required](https://img.shields.io/badge/rust-1.61+-blue.svg?label=Required%20Rust)

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

Specify `--features portable-pty` in the installation command 
to enable the pseudo-terminal (PTY) support (note that PTY capturing still needs
to be explicitly switched on when running `term-transcript` commands).
Without this feature, console app output is captured via OS pipes,
which means that programs dependent on [`isatty`] checks
or getting term size can produce different output than if launched in an actual shell
(no coloring, no line wrapping etc.).

## Usage

- The `capture` subcommand captures output from stdin, renders it to SVG and
  outputs SVG to stdout.
- The `exec` subcommand executes one or more commands in the shell, captures
  their outputs, renders to an SVG image and outputs it to stdout.
- The `test` subcommand allows testing snapshots from the command line.
- The `print` subcommand parses an SVG snapshot and outputs it to the command line.

Launch the CLI with the `--help` option for more details about arguments
for each subcommand.

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
[rainbow-script-link]: https://github.com/slowli/term-transcript/blob/HEAD/cli/rainbow.sh
[test-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/test.svg?sanitize=true
[test-color-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/test-fail.svg?sanitize=true
[test-link]: https://github.com/slowli/term-transcript/blob/HEAD/cli/tests/e2e.rs
[help-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/help.svg?sanitize=true
[`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
