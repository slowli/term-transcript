//! Simple REPL application that echoes the input with coloring / styles applied.

use std::io::{self, BufRead, Write};

use anstyle::{AnsiColor, Color, Style};
use term_transcript::svg::RgbColor;

fn process_line(writer: &mut impl Write, line: &str) -> io::Result<()> {
    let parts: Vec<_> = line.split_whitespace().collect();
    let mut style = Style::new();
    let mut intense = false;
    for (i, &part) in parts.iter().enumerate() {
        match part {
            "bold" => {
                style = style.bold();
            }
            "italic" => {
                style = style.italic();
            }
            "underline" => {
                style = style.underline();
            }
            "intense" => {
                intense = true;
            }

            "black" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Black)));
            }
            "blue" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Blue)));
            }
            "green" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Green)));
            }
            "red" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Red)));
            }
            "cyan" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
            }
            "magenta" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Magenta)));
            }
            "yellow" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
            }
            "white" => {
                style = style.fg_color(Some(Color::Ansi(AnsiColor::White)));
            }

            color if color.starts_with('#') => {
                if let Ok(color) = color.parse::<RgbColor>() {
                    let color = anstyle::RgbColor(color.0, color.1, color.2);
                    style = style.fg_color(Some(Color::Rgb(color)));
                }
            }

            _ => { /* Do nothing. */ }
        }

        if let Some(Color::Ansi(color)) = style.get_fg_color() {
            style = style.fg_color(Some(color.bright(intense).into()));
        }

        write!(writer, "{style}{part}{style:#}")?;
        if i + 1 < parts.len() {
            write!(writer, " ")?;
        }
    }
    writeln!(writer)
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(line) = lines.next().transpose()? {
        process_line(&mut io::stdout(), &line)?;
    }
    Ok(())
}
