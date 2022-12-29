# term-transcript CLI

[![Build Status](https://github.com/slowli/term-transcript/workflows/CI/badge.svg?branch=master)](https://github.com/slowli/term-transcript/actions)
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

Alternatively, you may use the app Docker image [as described below](#using-docker-image).

### Crate feature: `portable-pty`

Specify `--features portable-pty` in the installation command 
to enable the pseudo-terminal (PTY) support (note that PTY capturing still needs
to be explicitly switched on when running `term-transcript` commands).
Without this feature, console app output is captured via OS pipes,
which means that programs dependent on [`isatty`] checks
or getting term size can produce different output than if launched in an actual shell
(no coloring, no line wrapping etc.).

### Crate feature: `tracing`

Specify `--features tracing` in the installation command to enable tracing
of the main performed operations. This could be useful for debugging purposes.
Tracing is performed with the `term_transcript::*` targets, mostly on the `DEBUG` level.
Tracing events are output to the stderr using [the standard subscriber][fmt-subscriber];
its filtering can be configured using the `RUST_LOG` env variable
(e.g., `RUST_LOG=term_transcript=debug`).

## Usage

- The `capture` subcommand captures output from stdin, renders it to SVG and
  outputs SVG to stdout.
- The `exec` subcommand executes one or more commands in the shell, captures
  their outputs, renders to an SVG image and outputs it to stdout.
- The `test` subcommand allows testing snapshots from the command line.
- The `print` subcommand parses an SVG snapshot and outputs it to the command line.

Launch the CLI app with the `--help` option for more details about arguments
for each subcommand.

### Using Docker image

As a lower-cost alternative to the local installation, you may install and use the CLI app
from the [GitHub Container registry](https://github.com/slowli/term-transcript/pkgs/container/term-transcript).
To run the app in a Docker container, use a command like

```shell
cat examples/rainbow.svg | \
  docker run -i --rm --env COLOR=always \
  ghcr.io/slowli/term-transcript:master \
  print -
```

Here, the `COLOR` env variable sets the coloring preference for the output,
and the `-` arg for the `print` subcommand instructs reading from stdin.

Running `exec` and `test` subcommands from a Docker container is more tricky
since normally this would require taking the entire environment for the executed commands
into the container. In order to avoid this, you can establish a bidirectional channel
with the host using [`nc`](https://linux.die.net/man/1/nc), which is pre-installed
in the Docker image:

```shell
docker run --rm -v /tmp/shell.sock:/tmp/shell.sock \
  ghcr.io/slowli/term-transcript:master \
  exec --shell nc --echoing --args=-U --args=/tmp/shell.sock 'ls -al'
```

Here, the complete shell command connects `nc` to the Unix domain socket
at `/tmp/shell.sock`, which is mounted to the container using the `-v` option.

On the host side, connecting the `bash` shell to the socket could look like this:

```shell
mkfifo /tmp/shell.fifo
cat /tmp/shell.fifo | bash -i 2>&1 | nc -lU /tmp/shell.sock > /tmp/shell.fifo &
```

Here, `/tmp/shell.fifo` is a FIFO pipe used to exchange data between `nc` and `bash`.
The drawback of this approach is that the shell executable 
would not run in a (pseudo-)terminal and thus could look differently (no coloring etc.).
To connect a shell in a pseudo-terminal, you can use [`socat`](http://www.dest-unreach.org/socat/doc/socat.html),
changing the host command as follows:

```shell
socat UNIX-LISTEN:/tmp/shell.sock,fork EXEC:"bash -i",pty,setsid,ctty,stderr &
```

TCP sockets can be used instead of Unix sockets, but are not recommended
if Unix sockets are available since they are less secure. Indeed, care should be taken
that the host "server" is not bound to a publicly accessible IP address, which
would create a remote execution backdoor to the host system. As usual, caveats apply;
e.g., one can spawn the shell in another Docker container connecting it and the `term-transcript`
container in a single Docker network. In this case, TCP sockets are secure and arguably
easier to use given Docker built-in DNS resolution machinery.

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

See also [a shell script][generate-snapshots] used in the "parent" `term-transcript`
crate to render examples; it uses all major commands and options of the CLI app.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `term-transcript` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions. 

[`term-transcript`]: https://crates.io/crates/term-transcript
[fmt-subscriber]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html
[rainbow-script-link]: https://github.com/slowli/term-transcript/blob/HEAD/cli/rainbow.sh
[test-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/test.svg?sanitize=true
[test-color-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/test-fail.svg?sanitize=true
[test-link]: https://github.com/slowli/term-transcript/blob/HEAD/cli/tests/e2e.rs
[help-snapshot-link]: https://github.com/slowli/term-transcript/raw/HEAD/cli/tests/snapshots/help.svg?sanitize=true
[`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
[generate-snapshots]: https://github.com/slowli/term-transcript/blob/HEAD/examples/generate-snapshots.sh
