//! Simple REPL application that echoes the input with coloring / styles applied.

use std::io::{self, BufRead};

use term_transcript::svg::RgbColor;
use termcolor::{Ansi, Color, ColorSpec, WriteColor};

fn process_line(writer: &mut impl WriteColor, line: &str) -> io::Result<()> {
    let parts: Vec<_> = line.split_whitespace().collect();
    let mut color_spec = ColorSpec::new();
    for (i, &part) in parts.iter().enumerate() {
        match part {
            "bold" => {
                color_spec.set_bold(true);
            }
            "italic" => {
                color_spec.set_italic(true);
            }
            "underline" => {
                color_spec.set_underline(true);
            }
            "intense" => {
                color_spec.set_intense(true);
            }

            "black" => {
                color_spec.set_fg(Some(Color::Black));
            }
            "blue" => {
                color_spec.set_fg(Some(Color::Blue));
            }
            "green" => {
                color_spec.set_fg(Some(Color::Green));
            }
            "red" => {
                color_spec.set_fg(Some(Color::Red));
            }
            "cyan" => {
                color_spec.set_fg(Some(Color::Cyan));
            }
            "magenta" => {
                color_spec.set_fg(Some(Color::Magenta));
            }
            "yellow" => {
                color_spec.set_fg(Some(Color::Yellow));
            }
            "white" => {
                color_spec.set_fg(Some(Color::White));
            }

            color if color.starts_with('#') => {
                if let Ok(color) = color.parse::<RgbColor>() {
                    color_spec.set_fg(Some(Color::Rgb(color.0, color.1, color.2)));
                }
            }

            _ => { /* Do nothing. */ }
        }

        writer.set_color(&color_spec)?;
        write!(writer, "{part}")?;
        writer.reset()?;
        if i + 1 < parts.len() {
            write!(writer, " ")?;
        }
    }
    writeln!(writer)
}

fn main() -> anyhow::Result<()> {
    let mut writer = Ansi::new(io::stdout());
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(line) = lines.next().transpose()? {
        process_line(&mut writer, &line)?;
    }
    Ok(())
}
