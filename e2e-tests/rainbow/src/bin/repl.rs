//! Simple REPL application that echoes the input with coloring / styles applied.

use std::io::{self, BufRead};

use anstyle::Style;
use term_style::parse_style;

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut style = Style::new();
    while let Some(line) = lines.next().transpose()? {
        style = parse_style(&line, &style)?;
        println!("{style}{line}{style:#}");
    }
    Ok(())
}
