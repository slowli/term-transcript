# Using Library

Add this to your `Crate.toml`:

```toml
[dependencies]
term-transcript = "0.4.0"
```

## Basic workflow

The code snippet below executes a single `echo` command in the default shell
(`sh` for *NIX, `cmd` for Windows), and captures the rendered transcript to stdout.

```rust
use term_transcript::{svg::Template, ShellOptions, Transcript, UserInput};
use std::str;

let transcript = Transcript::from_inputs(
    &mut ShellOptions::default(),
    vec![UserInput::command(r#"echo "Hello world!""#)],
)?;
let mut writer = vec![];
// ^ Any `std::io::Write` implementation will do, such as a `File`.
Template::default().render(&transcript, &mut writer)?;
println!("{}", str::from_utf8(&writer)?);
anyhow::Ok(())
```

## Use in CLI tests

TODO
