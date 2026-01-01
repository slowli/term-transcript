# Rainbow script

This directory contains [Unix](rainbow) and [Windows-compatible](rainbow.bat) scripts producing ANSI-styled outputs.
The script is included into `PATH` and used by high-level and end-to-end tests in the workspace:

- [Generating / testing CLI examples](../../src/tests.rs)
- [CLI end-to-end tests](../../tests/e2e.rs)
- [Other end-to-end tests](../../../e2e-tests)

## Regenerating outputs

The Windows batch script uses pregenerated `*.out` files to print stuff since batch scripting is *difficult*.
To regenerate `*.out` files, run [`regenerate-rainbow-out.sh`](regenerate-rainbow-out.sh).
