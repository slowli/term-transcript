# term-transcript CLI

`term-transcript` CLI app provides an almost feature-complete alternative to [the library](../library.md).
It allows capturing, printing and testing terminal snapshots.

## Usage

- [The `exec` subcommand](#exec-subcommand) executes one or more commands in the shell, captures
  their outputs, renders to an SVG image and outputs it to stdout.
- [The `capture` subcommand](#capture-subcommand) captures output from stdin, renders it to SVG and
  outputs SVG to stdout.
- [The `test` subcommand](#test-subcommand) allows testing snapshots from the command line.
- [The `print` subcommand](#print-subcommand) parses an SVG snapshot and outputs it to the command line.

Launch the CLI app with the `--help` option for more details about arguments
for each subcommand. See also the [FAQ](../FAQ.md) for some tips and troubleshooting advice.

### `exec` subcommand

`term-transcript exec` sends one or more inputs to the customizable shell (e.g., `sh`)
and captures the produced outputs, including ANSI styling, into a snapshot.
The snapshot uses the SVG format by default, however, this can be customized
(see [the *Custom Template* section](../examples/custom-config.md#custom-template) for details).

> [!TIP]
>
> See [*Examples*](../examples) for various representation options that can be customized
> via command-line arguments.

### `capture` subcommand

`term-transcript capture` is quite similar to `term-transcript exec`, but instead of instantiating
a shell, it captures input from another command via shell pipelining.

![Example of `term-transcript capture` usage](../assets/subcommand-capture.svg)

### `print` subcommand

`term-transcript print` parses a previously captured transcript and outputs it to stdout,
applying the corresponding styles as necessary.

![Example of `term-transcript print` usage](../assets/subcommand-print.svg)

### `test` subcommand

`term-transcript test` reproduces inputs recorded in a captured transcript and compares outputs
to the ones recorded in the transcript.

![Example of `term-transcript test` usage](../assets/subcommand-test.svg)

If there's a test failure a diff will be produced highlighting the changes.
This includes discrepancies in ANSI styling if the `--precise` arg is provided, like in the snapshot below.

![Example of `term-transcript test` output with output mismatch](../assets/subcommand-test-fail.svg)

> [!TIP]
>
> See also [using the library for CLI testing](../library.md#use-in-cli-tests). This provides a more customizable alternative
> (e.g., allows to generate snapshots the first time the tests are run).

## Installation options

- [Use a pre-built binary](#downloads) for popular targets (x86_64 for Linux / macOS / Windows
  and AArch64 for macOS) from the `master` branch.
- Use a pre-built binary for popular targets from [GitHub Releases](https://github.com/slowli/term-transcript/releases).
- [Use the app Docker image](docker.md).
- [Build from sources](build.md) using Rust / `cargo`.

## Downloads

> [!IMPORTANT]
>
> The binaries are updated on each push to the git repo branch. Hence, they may contain more bugs
> than the release binaries mentioned above.

| Platform | Architecture | Download link                                                                                                     |
|:---------|:-------------|:------------------------------------------------------------------------------------------------------------------|
| Linux    | x86_64, GNU  | [<i class="fa-solid fa-download"></i> Download](../assets/term-transcript-master-x86_64-unknown-linux-gnu.tar.gz) |
| macOS    | x86_64       | [<i class="fa-solid fa-download"></i> Download](../assets/term-transcript-master-x86_64-apple-darwin.tar.gz)      |
| macOS    | arm64        | [<i class="fa-solid fa-download"></i> Download](../assets/term-transcript-master-aarch64-apple-darwin.tar.gz)     |
| Windows  | x86_64       | [<i class="fa-solid fa-download"></i> Download](../assets/term-transcript-master-x86_64-pc-windows-msvc.zip)      |
