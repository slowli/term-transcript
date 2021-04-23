use termcolor::{Color, ColorSpec, WriteColor};

use std::str;

use crate::TermError;

/// Parses terminal output and issues corresponding commands to the `writer`.
#[derive(Debug)]
pub struct TermOutputParser<'a, W> {
    writer: &'a mut W,
}

impl<'a, W: WriteColor> TermOutputParser<'a, W> {
    pub fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }

    pub fn parse(&mut self, term_output: &[u8]) -> Result<(), TermError> {
        const ANSI_ESC: u8 = 0x1b;
        const ANSI_CSI: u8 = b'[';

        let mut i = 0;
        let mut written_end = 0;
        while i < term_output.len() {
            if term_output[i] == ANSI_ESC {
                // Push preceding "ordinary" bytes into the writer.
                self.writer
                    .write_all(&term_output[written_end..i])
                    .map_err(TermError::Io)?;

                i += 1;
                let next_byte = term_output
                    .get(i)
                    .copied()
                    .ok_or(TermError::UnfinishedSequence)?;
                if next_byte == ANSI_CSI {
                    i += 1;
                    let csi = Csi::parse(&term_output[i..])?;
                    if let Some(color_spec) = csi.color_spec()? {
                        self.writer.set_color(&color_spec).map_err(TermError::Io)?;
                    }
                    i += csi.len;
                } else {
                    return Err(TermError::NonCsiSequence(next_byte));
                }
                written_end = i; // skip the escape sequence
            } else {
                // Ordinary char.
                i += 1;
            }
        }

        self.writer
            .write_all(&term_output[written_end..i])
            .map_err(TermError::Io)
    }
}

#[derive(Debug, Clone, Copy)]
struct Csi<'a> {
    parameters: &'a [u8],
    final_byte: u8,
    len: usize,
}

impl<'a> Csi<'a> {
    fn parse(buffer: &'a [u8]) -> Result<Self, TermError> {
        let intermediates_start = buffer
            .iter()
            .position(|byte| !(0x30..=0x3f).contains(byte))
            .ok_or(TermError::UnfinishedSequence)?;

        let final_byte_pos = buffer[intermediates_start..]
            .iter()
            .position(|byte| !(0x20..=0x2f).contains(byte))
            .ok_or(TermError::UnfinishedSequence)?;
        let final_byte_pos = intermediates_start + final_byte_pos;

        let final_byte = buffer[final_byte_pos];
        if (0x40..=0x7e).contains(&final_byte) {
            Ok(Self {
                parameters: &buffer[..intermediates_start],
                final_byte,
                len: final_byte_pos + 1,
            })
        } else {
            Err(TermError::InvalidSgrFinalByte(final_byte))
        }
    }

    fn color_spec(self) -> Result<Option<ColorSpec>, TermError> {
        if self.final_byte != b'm' {
            return Ok(None);
        }
        let mut spec = ColorSpec::new();
        spec.set_reset(false);

        let mut params = self.parameters.split(|&byte| byte == b';').peekable();
        if params.peek().is_none() {
            spec.set_reset(true);
        }
        while params.peek().is_some() {
            Self::process_param(&mut spec, &mut params)?;
        }
        Ok(Some(spec))
    }

    fn process_param(
        spec: &mut ColorSpec,
        mut params: impl Iterator<Item = &'a [u8]>,
    ) -> Result<(), TermError> {
        match params.next().unwrap() {
            b"0" => {
                spec.set_reset(true);
            }
            b"1" => {
                spec.set_bold(true);
            }
            b"2" => {
                spec.set_dimmed(true);
            }
            b"3" => {
                spec.set_italic(true);
            }
            b"4" => {
                spec.set_underline(true);
            }

            // TODO: 2x codes need more complex processing
            b"22" => {
                spec.set_bold(false).set_dimmed(false);
            }
            b"23" => {
                spec.set_italic(false);
            }
            b"24" => {
                spec.set_underline(false);
            }

            // Foreground colors
            b"30" => {
                spec.set_fg(Some(Color::Black));
            }
            b"31" => {
                spec.set_fg(Some(Color::Red));
            }
            b"32" => {
                spec.set_fg(Some(Color::Green));
            }
            b"33" => {
                spec.set_fg(Some(Color::Yellow));
            }
            b"34" => {
                spec.set_fg(Some(Color::Blue));
            }
            b"35" => {
                spec.set_fg(Some(Color::Magenta));
            }
            b"36" => {
                spec.set_fg(Some(Color::Cyan));
            }
            b"37" => {
                spec.set_fg(Some(Color::White));
            }
            b"38" => {
                let color = Self::read_color(params)?;
                spec.set_fg(Some(color));
            }
            b"39" => {
                spec.set_fg(None);
            }

            // Background colors
            b"40" => {
                spec.set_bg(Some(Color::Black));
            }
            b"41" => {
                spec.set_bg(Some(Color::Red));
            }
            b"42" => {
                spec.set_bg(Some(Color::Green));
            }
            b"43" => {
                spec.set_bg(Some(Color::Yellow));
            }
            b"44" => {
                spec.set_bg(Some(Color::Blue));
            }
            b"45" => {
                spec.set_bg(Some(Color::Magenta));
            }
            b"46" => {
                spec.set_bg(Some(Color::Cyan));
            }
            b"47" => {
                spec.set_bg(Some(Color::White));
            }
            b"48" => {
                let color = Self::read_color(params)?;
                spec.set_bg(Some(color));
            }
            b"49" => {
                spec.set_bg(None);
            }

            _ => { /* Do nothing */ }
        }
        Ok(())
    }

    fn read_color(mut params: impl Iterator<Item = &'a [u8]>) -> Result<Color, TermError> {
        let color_type = params.next().ok_or(TermError::UnfinishedColor)?;
        match color_type {
            b"5" => {
                let index = params.next().ok_or(TermError::UnfinishedColor)?;
                Self::parse_color_index(index).map(Color::Ansi256)
            }
            b"2" => {
                let r = params.next().ok_or(TermError::UnfinishedColor)?;
                let g = params.next().ok_or(TermError::UnfinishedColor)?;
                let b = params.next().ok_or(TermError::UnfinishedColor)?;

                let r = Self::parse_color_index(r)?;
                let g = Self::parse_color_index(g)?;
                let b = Self::parse_color_index(b)?;
                Ok(Color::Rgb(r, g, b))
            }
            _ => {
                let color_type = String::from_utf8_lossy(color_type).into_owned();
                Err(TermError::InvalidColorType(color_type))
            }
        }
    }

    fn parse_color_index(param: &[u8]) -> Result<u8, TermError> {
        if param.is_empty() {
            // As per ANSI standards, empty params are treated as number 0.
            return Ok(0);
        }

        let param = unsafe {
            // SAFETY: safe by construction; we've checked range of bytes in params
            // when creating a `Csi` instance.
            str::from_utf8_unchecked(param)
        };
        param.parse().map_err(TermError::InvalidColorIndex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use termcolor::Ansi;

    fn assert_eq_term_output(actual: &[u8], expected: &[u8]) {
        assert_eq!(
            String::from_utf8_lossy(actual),
            String::from_utf8_lossy(expected)
        );
    }

    #[test]
    fn term_roundtrip_simple() -> anyhow::Result<()> {
        let mut writer = Ansi::new(vec![]);
        write!(writer, "Hello, ")?;
        writer.set_color(ColorSpec::new().set_bold(true).set_fg(Some(Color::Green)))?;
        write!(writer, "world")?;
        writer.reset()?;
        write!(writer, "!")?;

        let term_output = writer.into_inner();

        let mut new_writer = Ansi::new(vec![]);
        TermOutputParser::new(&mut new_writer).parse(&term_output)?;
        let new_term_output = new_writer.into_inner();
        assert_eq_term_output(&new_term_output, &term_output);
        Ok(())
    }

    #[test]
    fn term_roundtrip_with_multiple_colors() -> anyhow::Result<()> {
        let mut writer = Ansi::new(vec![]);
        write!(writer, "He")?;
        writer.set_color(
            ColorSpec::new()
                .set_bg(Some(Color::White))
                .set_fg(Some(Color::Black)),
        )?;
        write!(writer, "ll")?;
        writer.reset()?;
        writer.set_color(
            ColorSpec::new()
                .set_intense(true)
                .set_fg(Some(Color::Magenta)),
        )?;
        write!(writer, "o")?;
        writer.reset()?;
        writer.set_color(
            ColorSpec::new()
                .set_italic(true)
                .set_fg(Some(Color::Green))
                .set_bg(Some(Color::Yellow)),
        )?;
        write!(writer, "world")?;
        writer.reset()?;
        writer.set_color(
            ColorSpec::new()
                .set_underline(true)
                .set_dimmed(true)
                .set_bg(Some(Color::Cyan)),
        )?;
        write!(writer, "!")?;

        let term_output = writer.into_inner();

        let mut new_writer = Ansi::new(vec![]);
        TermOutputParser::new(&mut new_writer).parse(&term_output)?;
        let new_term_output = new_writer.into_inner();
        assert_eq_term_output(&new_term_output, &term_output);
        Ok(())
    }

    #[test]
    fn roundtrip_with_indexed_colors() -> anyhow::Result<()> {
        let mut writer = Ansi::new(vec![]);
        write!(writer, "H")?;
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(5))))?;
        write!(writer, "e")?;
        writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(11))))?;
        write!(writer, "l")?;
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(33))))?;
        write!(writer, "l")?;
        writer.set_color(ColorSpec::new().set_bg(Some(Color::Ansi256(250))))?;
        write!(writer, "o")?;

        let term_output = writer.into_inner();

        let mut new_writer = Ansi::new(vec![]);
        TermOutputParser::new(&mut new_writer).parse(&term_output)?;
        let new_term_output = new_writer.into_inner();
        assert_eq_term_output(&new_term_output, &term_output);
        Ok(())
    }

    #[test]
    fn roundtrip_with_rgb_colors() -> anyhow::Result<()> {
        let mut writer = Ansi::new(vec![]);
        write!(writer, "H")?;
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(16, 22, 35))))?;
        write!(writer, "e")?;
        writer.set_color(ColorSpec::new().set_bg(Some(Color::Rgb(255, 254, 253))))?;
        write!(writer, "l")?;
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(0, 0, 0))))?;
        write!(writer, "l")?;
        writer.set_color(ColorSpec::new().set_bg(Some(Color::Rgb(0, 160, 128))))?;
        write!(writer, "o")?;

        let term_output = writer.into_inner();

        let mut new_writer = Ansi::new(vec![]);
        TermOutputParser::new(&mut new_writer).parse(&term_output)?;
        let new_term_output = new_writer.into_inner();
        assert_eq_term_output(&new_term_output, &term_output);
        Ok(())
    }
}
