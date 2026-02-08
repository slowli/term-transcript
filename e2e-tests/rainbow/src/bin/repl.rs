//! Simple REPL application that echoes the input with coloring / styles applied.

use std::io::{self, BufRead};

use anstyle::{AnsiColor, Color, Style};
use styled_str::RichStyle;

const ERR: Style = Style::new()
    .bold()
    .fg_color(Some(Color::Ansi(AnsiColor::BrightRed)));
const STR: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut style = Style::new();
    while let Some(line) = lines.next().transpose()? {
        let parts: Vec<_> = line.split(';').collect();
        for (i, &part) in parts.iter().enumerate() {
            style = match RichStyle::parse(part, &style) {
                Ok(style) => style,
                Err(err) => {
                    if i > 0 {
                        println!(); // Flush the previously accumulated buffer
                    }
                    eprintln!("{ERR}error{ERR:#} parsing {STR}{part:?}{STR:#}: {err}");
                    break;
                }
            };
            print!("{style}{}{style:#}", RichStyle(&style));
            if i + 1 < parts.len() {
                print!("; ");
            } else {
                println!();
            }
        }
    }
    Ok(())
}
