# Examples

This section showcases various `term-transcript` options. It uses [the CLI app](../cli)
as more approachable, but all showcased features work in [the Rust library](../library.md) as well.

## Command-line args

| Section                               | Covered command-line args                                                          |
|:--------------------------------------|:-----------------------------------------------------------------------------------|
| [*Basics*](basics.md)                 | `--palette`, `--pure-svg`, `--pty`, `--scroll`, `--line-height`, `--advance-width` |
| [*Window Appearance*](window.md)      | `--width`, `--hard-wrap`, `--scroll-interval`, `--scroll-len`, `--window`          |
| [*Line Numbering*](line-numbering.md) | `--line-numbers`, `--continued-mark`, `--hard-wrap-mark`                           |
| [*Custom Fonts*](fonts.md)            | `--font`, `--embed-font`                                                           |
| [*Input Control*](input-control.md)   | `--no-inputs`                                                                      |
| [*Custom Config*](custom-config.md)   | `--tpl`, `--config-path`                                                           |                

## `rainbow` script

Most examples use `rainbow` -- a shell script showcasing various ANSI styles.

<details>
<summary><strong>rainbow shell script</strong> (click to expand)</summary>

```bash
{{#include ../../../e2e-tests/rainbow/bin/rainbow}}
```
</details>
