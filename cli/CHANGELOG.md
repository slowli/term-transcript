# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.3.0-beta.2 - 2023-04-29

*(All changes are relative compared to [the 0.3.0-beta.1 release](#030-beta1---2023-01-19))*

### Added

- Allow specifying the font family to be used in the generated SVG snapshots
  via the `--font` argument.
- Allow specifying additional CSS instructions for the generated SVG snapshots
  using the `--styles` argument. 
  As an example, this can be used to import fonts using `@import` or `@font-face`.
- Support rendering pure SVG using `--pure-svg` option. See the library changelog and FAQ
  for more details.
- Allow hiding all user inputs in a rendered transcript by specifying the `--no-inputs` flag.

## 0.3.0-beta.1 - 2023-01-19

### Added

- Add ability to customize the rendering template using `--tpl <path>` option.
  Additionally, `--tpl -` outputs JSON data that would be fed to a template
  (could be useful if complex data processing is required).
- Add the `--echoing` flag to specify whether the shell is echoing.
- Support exit status capturing if using the default shell or another supported shell
  (`sh`, `bash`, `powershell` or `pwsh`). See the `term-transcript` crate docs
  for more details on exit statuses.
- Print captured exit statuses in the `print` subcommand.
- Allow redefining the initialization timeout with the help of the `--init-timeout` / `-I` option.
- Proxy tracing from the `term-transcript` crate if the `tracing` crate feature is on.
- Support line numbering with the help of the `--line-numbers` / `-n` option.
- Add a Docker image for the CLI app
  on the [GitHub Container registry](https://github.com/slowli/term-transcript/pkgs/container/term-transcript).
- Add prebuilt binaries for popular targets (x86_64 for Linux / macOS / Windows
  and aarch64 for macOS) available from [GitHub releases](https://github.com/slowli/term-transcript/releases).

### Changed

- Change working directory to the working directory of the parent process
  for the `exec` subcommand.
- Use [`humantime`](https://docs.rs/humantime/) for UX-friendly timeout values
  (`--io-timeout` / `-T` and `--init-timeout` / `-I` options).

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
