# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add location info for transcript parsing errors and make error types public.
- Support parsing SVG transcripts generated with the pure SVG template. In particular, these transcripts
  are now supported in snapshot testing. The pure SVG template is slightly updated for parsing;
  thus, parsing won't work with transcripts produced with old `term-transcript` versions.
- Support embedding fonts into the SVG template via `@font-face` CSS rules with a data URL.
  Provide font subsetting as an extension via the opt-in `font-subset` feature.
- Allow configuring line height and char (advance) width for both HTML-in-SVG and pure SVG templates.

### Changed

- Update `quick-xml` dependency.
- Bump minimum supported Rust version to 1.83.
- Align the view box bottom during the last scroll animation frame, so that there's no overscroll.
- Consistently trim the ending newline for captured outputs.
- Change the hard break char to `»` so that it is covered by more fonts. Do not style the hard break as the surrounding text.
- Change the output data provided to templates. Instead of pre-rendered HTML and SVG data, a template is now provided
  with an array of lines, each consisting of styled text spans.
- Decrease the default line height to 1.2 (i.e., 16.8px); previously, it was 18px (i.e., ~1.29).
- Change the way background fill works for HTML-in-SVG so that it always has the full line height.

### Fixed

- Fix background fill for pure SVG, so that it doesn't rely on full block chars. Correspondingly, it now works
  even if these chars are not present in the selected fonts.
- Fix most compatibility issues with Safari / iOS WebView.

### Removed

- Remove the `Parsed::html()` getter as difficult to maintain given pure SVG parsing.

## 0.4.0 - 2025-06-01

*(All changes are relative compared to the [0.4.0-beta.1 release](#040-beta1---2024-03-03))*

### Added

- Allow transforming captured `Transcript`s. This is mostly useful for testing to filter out / replace
  variable / env-dependent output parts. Correspondingly, `TestConfig` allows customizing a transform
  using `with_transform()` method.

### Changed

- Update `quick-xml` and `handlebars` dependencies.
- Bump minimum supported Rust version to 1.74.

### Fixed

- Fix rendering errors with standard templates with newer versions of `handlebars`.

## 0.4.0-beta.1 - 2024-03-03

### Changed

- Allow configuring pixels per scroll using new `ScrollOptions.pixels_per_scroll` field.
- Change some default values and set more default values during `TemplateOptions` deserialization.
- Bump minimum supported Rust version to 1.70.
- Update `handlebars` and `quick-xml` dependencies.

## 0.3.0 - 2023-06-03

*(No substantial changes compared to the [0.3.0-beta.2 release](#030-beta2---2023-04-29))*

## 0.3.0-beta.2 - 2023-04-29

*(All changes are relative compared to [the 0.3.0-beta.1 release](#030-beta1---2023-01-19))*

### Added

- Add a pure SVG rendering option to `svg::Template`. Since rendered SVGs do not contain
  embedded HTML, they are supported by more SVG viewers / editors (e.g., Inkscape).
  On the downside, the rendered SVG may have mispositioned background text coloring
  in certain corner cases.
- Allow specifying additional CSS instructions in `svg::TemplateOptions`.
  As an example, this can be used to import fonts using `@import` or `@font-face`.
- Add a fallback error message to the default template if HTML-in-SVG embedding
  is not supported.
- Add [FAQ](../FAQ.md) with some tips and troubleshooting advice.
- Allow hiding `UserInput`s during transcript rendering by calling the `hide()` method.
  Hidden inputs are supported by the default and pure SVG templates.

### Changed

- Update `portable-pty` and `quick-xml` dependencies.
- Bump minimum supported Rust version to 1.66.

## 0.3.0-beta.1 - 2023-01-19

### Added

- Support custom rendering templates via `Template::custom()`. 
  This allows customizing rendering logic, including changing the output format
  entirely (e.g., to HTML).
- Allow capturing exit statuses of commands executed in the shell.
- Trace major operations using the [`tracing`](https://docs.rs/tracing/) facade.
- Support line numbering for the default SVG template.

### Changed

- Update `quick-xml` dependency.
- Bump minimum supported Rust version to 1.61.
- Replace a line mapper in `ShellOptions` to a more general line decoder that can handle
  non-UTF-8 encodings besides input filtering.
- Improve configuring echoing in `ShellOptions`.
- Use the initialization timeout from `ShellOptions` for each command, not only for
  the first command. This allows reducing the I/O timeout and thus performing operations faster.

## 0.2.0 - 2022-06-12

*(All changes are relative compared to [the 0.2.0-beta.1 release](#020-beta1---2022-01-06))*

### Changed

- Update `quick-xml` dependency.
- Bump minimum supported Rust version to 1.57 and switch to 2021 Rust edition.

### Fixed

- Properly handle non-ASCII input when parsing `RgbColor`.

### Removed

- Remove `From<&&str>` implementation for `UserInput`. This implementation was previously used
  to make `Transcript::from_inputs()` and `TestConfig::test()` accept user inputs as `&[&str]`.
  In Rust 2021 edition, it is possible to use arrays (`[&str; _]`) instead.

## 0.2.0-beta.1 - 2022-01-06

### Added

- Support interacting with shell using pseudo-terminal (PTY) via `portable-pty`
  crate.
- Add `ShellOptions::with_env()` to set environment variables for the shell.
- Make style / color comparisons more detailed and human-readable.
- Allow specifying initialization timeout for `ShellOptions`. This timeout
  is added to the I/O timeout to wait for output for the first command.
- Add `TestConfig::test()` to perform more high-level / fluent snapshot testing.
- Allow adding generic paths to the `PATH` env var for the spawned shell
  via `ShellOptions::with_additional_path()`.

### Changed

- Update `handlebars` and `pretty_assertions` dependencies.
- Generalize `TermError::NonCsiSequence` variant to `UnrecognizedSequence`.
- Make `TestConfig` modifiers take `self` by value for the sake of fluency.

### Fixed

- Fix flaky PowerShell initialization that could lead to the init command
  being included into the captured output.
- Fix parsing of `90..=97` and `100..=107` SGR params (i.e., intense foreground
  and background colors).
- Enable parsing OSC escape sequences; they are now ignored instead of leading
  to a `TermError`.
- Process carriage return `\r` in terminal output. (As a stopgap measure, the text
  before `\r` is not rendered.)
- Fix rendering intense colors into HTML. Previously, intense color marker
  was dropped in certain cases.
- Fix waiting for echoed initialization commands.
- Add `height` attribute to top-level SVG to fix its rendering.
- Remove an obsolete lifetime parameter from `svg::Template` and change `Template::render`
  to receive `self` by shared reference.
- Fix `TestConfig` output not being captured during tests.

## 0.1.0 - 2021-06-01

The initial release of `term-transcript`.
