# Building from Sources

To build the CLI app from sources, run:

```bash
cargo install --locked term-transcript-cli
# Optionally, specify the release `--version`, or `--git` + 
# `--tag` / `--branch` / `--rev`  to build from the git repo.

# This will install `term-transcript` executable, which can be checked
# as follows:
term-transcript --help
```

This requires a Rust toolchain locally installed.

### Minimum supported Rust version

The crate supports the latest stable Rust version. It may support previous stable Rust versions,
but this is not guaranteed.

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

[`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
[fmt-subscriber]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html
