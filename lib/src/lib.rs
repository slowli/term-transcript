//! Snapshot testing for CLI / REPL applications, in a fun way.
//!
//! # What it does
//!
//! This crate allows to:
//!
//! - Create [`Transcript`]s of interacting with a terminal, capturing both the output text
//!   and [ANSI-compatible color info][SGR].
//! - Save these transcripts in the [SVG] format, so that they can be easily embedded as images
//!   into HTML / Markdown documents. (Output format customization
//!   [is also supported](svg::Template#customization) via [Handlebars] templates.)
//! - Parse transcripts from SVG
//! - Test that a parsed transcript actually corresponds to the terminal output (either as text
//!   or text + colors).
//!
//! The primary use case is easy to create and maintain end-to-end tests for CLI / REPL apps.
//! Such tests can be embedded into a readme file.
//!
//! # Design decisions
//!
//! - **Static capturing.** Capturing dynamic interaction with the terminal essentially
//!   requires writing / hacking together a new terminal, which looks like an overkill
//!   for the motivating use case (snapshot testing).
//!
//! - **(Primarily) static SVGs.** Animated SVGs create visual noise and make simple things
//!   (e.g., copying text from an SVG) harder than they should be.
//!
//! - **Self-contained tests.** Unlike generic snapshot files, [`Transcript`]s contain
//!   both user inputs and outputs. This allows using them as images with little additional
//!   explanation.
//!
//! # Limitations
//!
//! - Terminal coloring only works with ANSI escape codes. (Since ANSI escape codes
//!   are supported even on Windows nowadays, this shouldn't be a significant problem.)
//! - ANSI escape sequences other than [SGR] ones are either dropped (in case of [CSI]
//!   and OSC sequences), or lead to [`TermError::UnrecognizedSequence`].
//! - By default, the crate exposes APIs to perform capture via OS pipes.
//!   Since the terminal is not emulated in this case, programs dependent on [`isatty`] checks
//!   or getting term size can produce different output than if launched in an actual shell
//!   (no coloring, no line wrapping etc.).
//! - It is possible to capture output from a pseudo-terminal (PTY) using the `portable-pty`
//!   crate feature. However, since most escape sequences are dropped, this is still not a good
//!   option to capture complex outputs (e.g., ones moving cursor).
//!
//! # Alternatives / similar tools
//!
//! - [`insta`](https://crates.io/crates/insta) is a generic snapshot testing library, which
//!   is amazing in general, but *kind of* too low-level for E2E CLI testing.
//! - [`rexpect`](https://crates.io/crates/rexpect) allows testing CLI / REPL applications
//!   by scripting interactions with them in tests. It works in Unix only.
//! - [`trybuild`](https://crates.io/crates/trybuild) snapshot-tests output
//!   of a particular program (the Rust compiler).
//! - [`trycmd`](https://crates.io/crates/trycmd) snapshot-tests CLI apps using
//!   a text-based format.
//! - Tools like [`termtosvg`](https://github.com/nbedos/termtosvg) and
//!   [Asciinema](https://asciinema.org/) allow recording terminal sessions and save them to SVG.
//!   The output of these tools is inherently *dynamic* (which, e.g., results in animated SVGs).
//!   This crate [intentionally chooses](#design-decisions) a simpler static format, which
//!   makes snapshot testing easier.
//!
//! # Crate features
//!
//! ## `portable-pty`
//!
//! *(Off by default)*
//!
//! Allows using pseudo-terminal (PTY) to capture terminal output rather than pipes.
//! Uses [the eponymous crate][`portable-pty`] under the hood.
//!
//! ## `svg`
//!
//! *(On by default)*
//!
//! Exposes [the eponymous module](svg) that allows rendering [`Transcript`]s
//! into the SVG format.
//!
//! ## `font-subset`
//!
//! *(Off by default)*
//!
//! Enables subsetting and embedding OpenType fonts into snapshots. Requires the `svg` feature.
//!
//! ## `test`
//!
//! *(On by default)*
//!
//! Exposes [the eponymous module](crate::test) that allows parsing [`Transcript`]s
//! from SVG files and testing them.
//!
//! ## `pretty_assertions`
//!
//! *(On by default)*
//!
//! Uses [the eponymous crate][`pretty_assertions`] when testing SVG files.
//! Only really makes sense together with the `test` feature.
//!
//! ## `tracing`
//!
//! *(Off by default)*
//!
//! Uses [the eponymous facade][`tracing`] to trace main operations, which could be useful
//! for debugging. Tracing is mostly performed on the `DEBUG` level.
//!
//! [SVG]: https://developer.mozilla.org/en-US/docs/Web/SVG
//! [SGR]: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR
//! [CSI]: https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences
//! [`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
//! [Handlebars]: https://handlebarsjs.com/
//! [`pretty_assertions`]: https://docs.rs/pretty_assertions/
//! [`portable-pty`]: https://docs.rs/portable-pty/
//! [`tracing`]: https://docs.rs/tracing/
//!
//! # Examples
//!
//! Creating a terminal [`Transcript`] and rendering it to SVG.
//!
//! ```
//! use term_transcript::{
//!     svg::{Template, TemplateOptions}, ShellOptions, Transcript, UserInput,
//! };
//! # use std::str;
//!
//! # fn main() -> anyhow::Result<()> {
//! let transcript = Transcript::from_inputs(
//!     &mut ShellOptions::default(),
//!     vec![UserInput::command(r#"echo "Hello world!""#)],
//! )?;
//! let mut writer = vec![];
//! // ^ Any `std::io::Write` implementation will do, such as a `File`.
//! Template::default().render(&transcript, &mut writer)?;
//! println!("{}", str::from_utf8(&writer)?);
//! # Ok(())
//! # }
//! ```
//!
//! Snapshot testing. See the [`test` module](crate::test) for more examples.
//!
//! ```no_run
//! use term_transcript::{test::TestConfig, ShellOptions};
//!
//! #[test]
//! fn echo_works() {
//!     TestConfig::new(ShellOptions::default()).test(
//!         "tests/__snapshots__/echo.svg",
//!         &[r#"echo "Hello world!""#],
//!     );
//! }
//! ```

// Documentation settings.
#![doc(html_root_url = "https://docs.rs/term-transcript/0.5.0-beta.1")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "portable-pty")]
pub use self::pty::{PtyCommand, PtyShell};
pub use self::{
    shell::{ShellOptions, StdShell},
    types::{ExitStatus, Interaction, TermError, Transcript, UserInput},
};

#[cfg(feature = "portable-pty")]
mod pty;
mod shell;
//mod style;
#[cfg(feature = "svg")]
#[cfg_attr(docsrs, doc(cfg(feature = "svg")))]
pub mod svg;
#[cfg(feature = "test")]
#[cfg_attr(docsrs, doc(cfg(feature = "test")))]
pub mod test;
pub mod traits;
mod types;
mod utils;

#[cfg(doctest)]
doc_comment::doctest!("../README.md");
