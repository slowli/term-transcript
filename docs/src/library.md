# Using Library

Add this to your `Crate.toml`:

```toml
[dependencies]
term-transcript = "0.4.0"
```

## Basic workflow

The code snippet below executes a single `echo` command in the default shell
(`sh` for *NIX, `cmd` for Windows), and captures the rendered transcript to stdout.

{{#include ../../lib/README.md:example}}

## Use in CLI tests

CLI tests are effectively *slightly* more sophisticated snapshot tests. Such tests usually adhere
to the following workflow.

> [!TIP]
>
> The snippets below are taken from end-to-end tests for `term-transcript` CLI.

### Define path to snapshots

For example, snapshots may be located in the `examples` directory of the crate,
or in a `tests` subdirectory.

```rust,ignore
{{#include ../../cli/tests/e2e.rs:snapshots_path}}
```

### Configure shell

This configures the used shell (e.g., `sh` or `bash`), the working directory, `PATH` additions etc.
Usually can be shared among all tests.

```rust,ignore
{{#include ../../cli/tests/e2e.rs:config}}
```

### Configure template(s)

Zero or more template options determining how the captured snapshots are displayed, e.g.,
[scrolling options](examples/window.md#scroll-height), [window frame](examples/window.md#window-frame-and-title),
[line numbering](examples/line-numbering.md) etc.

```rust,ignore
{{#include ../../cli/tests/e2e.rs:template}}
```

### Define tests

Finally, use the definitions above for tests. Each test will provide inputs supplied to the shell,
and will compare the captured output to one recorded in the snapshot.

```rust,ignore
{{#include ../../cli/tests/e2e.rs:simple_test}}
```
