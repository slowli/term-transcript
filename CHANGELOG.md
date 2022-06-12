# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.2.0 - 2022-06-12

*(All changes are relative compared to [the 0.2.0-beta.1 release](#020-beta1---2022-01-06))*

### Changed

- Update `quick-xml` dependency.
- Bump minimum supported Rust version and switch to 2021 Rust edition.

### Fixed

- Properly handle non-ASCII input when parsing `RgbColor`.

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
