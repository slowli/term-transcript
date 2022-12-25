# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add ability to customize the rendering template using `--tpl <path>` option.
  Additionally, `--tpl -` outputs JSON data that would be fed to a template
  (could be useful if complex data processing is required).
- Add the `--echoing` flag to specify whether the shell is echoing.
- Support exit status capturing if using the default shell or another supported shell
  (`sh`, `bash`, `powershell` or `pwsh`). See the `term-transcript` crate docs
  for more details on exit statuses.
- Print captured exit statuses in the `print` subcommand.

### Changed

- Change working directory to the working directory of the parent process
  for the `exec` subcommand.

## 0.2.0 - 2022-06-12

*(All changes are relative compared to [the 0.2.0-beta.1 release](#020-beta1---2022-01-06))*

### Changed

- Bump minimum supported Rust version and switch to 2021 Rust edition.

## 0.2.0-beta.1 - 2022-01-06

### Added

- Add `print` command to parse the specified SVG snapshot and print it to the shell.

### Fixed

- Remove obsolete dependencies.

## 0.1.0 - 2021-06-01

The initial release of `term-transcript-cli`.
