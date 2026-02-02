# Introduction

`term-transcript` is a Rust library and a CLI app that allow to:

- Create transcripts of interacting with a terminal, capturing both the output text
  and [ANSI-compatible color info][SGR].
- Save these transcripts in the [SVG] format, so that they can be easily embedded as images
  into HTML / Markdown documents. Rendering logic can be customized via [Handlebars] template engine;
  thus, other output formats besides SVG (e.g., HTML) are possible.
- Parse transcripts from SVG.
- Test that a parsed transcript actually corresponds to the terminal output (either as text
  or text + colors).

The primary use case is easy to create and maintain end-to-end tests for CLI / REPL apps.
Such tests can be embedded into a readme file.

## Usage

`term-transcript` comes in two flavors: a [Rust library](library.md), and a [CLI app](cli/README.md).
The CLI app has slightly less functionality, but does not require Rust knowledge.
See their docs and the [FAQ](FAQ.md) for usage guidelines and troubleshooting advice.

## Examples

An SVG snapshot of [the test `rainbow` script](examples/rainbow/rainbow)
produced by this crate:

![Snapshot of rainbow example](assets/rainbow.svg)

A snapshot of the same example with the scrolling animation and window frame:

![Animated snapshot of rainbow example](assets/animated.svg)

See the [CLI examples](examples) for more snapshot examples.

## Limitations

- Terminal coloring only works with ANSI escape codes. (Since ANSI escape codes
  are supported even on Windows nowadays, this shouldn't be a significant problem.)
- ANSI escape sequences other than [SGR] ones are either dropped (in case of [CSI] sequences),
  or lead to an error.
- By default, the crate exposes APIs to perform capture via OS pipes.
  Since the terminal is not emulated in this case, programs dependent on [`isatty`] checks
  or getting term size can produce different output than if launched in an actual shell
  (no coloring, no line wrapping etc.).
- It is possible to capture output from a pseudo-terminal (PTY) using the `portable-pty`
  crate feature. However, since most escape sequences are dropped, this is still not a good
  option to capture complex outputs (e.g., ones moving cursor).
- PTY support for Windows is shaky. It requires a somewhat recent Windows version
  (Windows 10 from October 2018 or newer), and may work incorrectly even for the recent versions.

[SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
[Handlebars]: https://handlebarsjs.com/
[SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
[CSI]: https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences
[`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
