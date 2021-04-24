# Snapshot Testing for CLI / REPL Applications

This crate allows to:

- Create transcripts of interacting with a terminal, capturing both the output text
  and [ANSI-compatible color info][SGR].
- Save these transcripts in the [SVG] format, so that they can be easily embedded as images
  into HTML / Markdown documents
- Parse transcripts from SVG
- Test that a parsed transcript actually corresponds to the terminal output (either as text
  or text + colors).

The primary use case is easy to create and maintain end-to-end tests for CLI / REPL apps.
Such tests can be embedded into a readme file.

## Usage

Add this to your `Crate.toml`:

```toml
[dependencies]
term-transcript = "0.1.0"
```

Example of usage:

```rust
use term_transcript::{
    svg::{Template, TemplateOptions}, ShellOptions, Transcript, UserInput,
};
use std::str;

let transcript = Transcript::from_inputs(
    &mut ShellOptions::default(),
    vec![UserInput::command(r#"echo "Hello world!""#)],
)?;
let mut writer = vec![];
// ^ Any `std::io::Write` implementation will do, such as a `File`.
Template::new(TemplateOptions::default()).render(&transcript, &mut writer)?;
println!("{}", str::from_utf8(&writer)?);
Ok::<_, anyhow::Error>(())
```

See more examples in the crate docs.

### Snapshot examples

Here's an SVG snapshot of [the `rainbow` example](examples/rainbow.rs)
produced by this crate:

![snapshot of rainbow example](tests/snapshots/rainbow.svg)

# Alternatives / similar tools

- [`insta`](https://crates.io/crates/insta) is a generic snapshot testing library, which
  is amazing in general, but *kind of* too low-level for E2E CLI testing.
- [`trybuild`](https://crates.io/crates/trybuild) snapshot-tests output
  of a particular program (the Rust compiler).
- Tools like [`termtosvg`](https://github.com/nbedos/termtosvg) and
  [Asciinema](https://asciinema.org/) allow recording terminal sessions and save them to SVG.
  The output of these tools is inherently *dynamic* (which, e.g., results in animated SVGs).
  This crate intentionally chooses a simpler static format, which makes snapshot testing easier.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `term-transcript` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
[SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
