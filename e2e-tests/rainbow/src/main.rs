//! Simple executable that outputs colored output. Used for testing.

use std::{
    env,
    io::{self, Write},
};

use termcolor::{Ansi, Color, ColorSpec, WriteColor};

const BASE_COLORS: &[(&str, Color)] = &[
    ("black", Color::Black),
    ("blue", Color::Blue),
    ("green", Color::Green),
    ("red", Color::Red),
    ("cyan", Color::Cyan),
    ("magenta", Color::Magenta),
    ("yellow", Color::Yellow),
];

const RGB_COLORS: &[(&str, Color)] = &[
    ("pink", Color::Rgb(0xff, 0xbb, 0xdd)),
    ("orange", Color::Rgb(0xff, 0xaa, 0x44)),
    ("brown", Color::Rgb(0x9f, 0x40, 0x10)),
    ("teal", Color::Rgb(0x10, 0x88, 0x9f)),
];

fn write_base_colors(
    writer: &mut impl WriteColor,
    intense: bool,
    long_lines: bool,
) -> anyhow::Result<()> {
    for (i, &(name, color)) in BASE_COLORS.iter().enumerate() {
        let mut color_spec = ColorSpec::new();
        color_spec.set_fg(Some(color)).set_intense(intense);
        if (i % 2 == 0) ^ intense {
            color_spec.set_underline(true);
        }
        writer.set_color(&color_spec)?;
        write!(writer, "{name}")?;
        writer.reset()?;
        write!(writer, " ")?;

        if long_lines {
            color_spec
                .set_underline(!color_spec.underline())
                .set_italic(true);
            writer.set_color(&color_spec)?;
            write!(writer, "{name}/italic")?;
            writer.reset()?;
            write!(writer, " ")?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

fn write_base_colors_bg(writer: &mut impl WriteColor, intense: bool) -> anyhow::Result<()> {
    for &(name, color) in BASE_COLORS {
        let mut color_spec = ColorSpec::new();
        color_spec
            .set_fg(Some(Color::White))
            .set_bg(Some(color))
            .set_intense(intense);
        writer.set_color(&color_spec)?;
        write!(writer, "{name}")?;
        writer.reset()?;
        write!(writer, " ")?;
    }
    writeln!(writer)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let long_lines = env::args().any(|arg| arg == "--long-lines");
    let short = env::args().any(|arg| arg == "--short");
    let mut writer = Ansi::new(io::stdout());

    writeln!(writer, "Base colors:")?;
    write_base_colors(&mut writer, false, long_lines)?;
    write_base_colors(&mut writer, true, long_lines)?;

    writeln!(writer, "Base colors (bg):")?;
    write_base_colors_bg(&mut writer, false)?;
    write_base_colors_bg(&mut writer, true)?;

    if short {
        return Ok(());
    }

    writeln!(writer, "ANSI color palette:")?;
    for color_idx in 16_u8..232 {
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(color_idx))))?;
        write!(writer, "!")?;

        let col = (color_idx - 16) % 36;
        let fg = if col < 16 { Color::White } else { Color::Black };
        writer.set_color(
            ColorSpec::new()
                .set_fg(Some(fg))
                .set_bg(Some(Color::Ansi256(color_idx))),
        )?;
        write!(writer, "?")?;

        if col == 35 && !long_lines {
            writer.reset()?;
            writeln!(writer)?;
        }
    }

    if long_lines {
        writer.reset()?;
        writeln!(writer)?;
    }

    writeln!(writer, "ANSI grayscale palette:")?;
    for color_idx in 232_u8..=255 {
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(color_idx))))?;
        write!(writer, "!")?;

        let fg = if color_idx < 244 {
            Color::White
        } else {
            Color::Black
        };
        writer.set_color(
            ColorSpec::new()
                .set_fg(Some(fg))
                .set_bg(Some(Color::Ansi256(color_idx)))
                .set_bold(true),
        )?;
        write!(writer, "?")?;
    }
    writer.reset()?;
    writeln!(writer)?;

    writeln!(writer, "24-bit colors:")?;
    for &(name, color) in RGB_COLORS {
        writer.set_color(ColorSpec::new().set_fg(Some(color)))?;
        write!(writer, "{name}")?;
        writer.reset()?;
        write!(writer, " ")?;
    }
    writeln!(writer)?;

    Ok(())
}
