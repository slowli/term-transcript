//! Parser for terminal output that converts it to a sequence of instructions to
//! a writer implementing `WriteColor`.

use std::str;

use termcolor::{Color, ColorSpec, WriteColor};

use crate::TermError;

/// Parses terminal output and issues corresponding commands to the `writer`.
#[derive(Debug)]
pub(crate) struct TermOutputParser<'a, W> {
    writer: &'a mut W,
    color_spec: ColorSpec,
}

impl<'a, W: WriteColor> TermOutputParser<'a, W> {
    pub(crate) fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            color_spec: ColorSpec::new(),
        }
    }

    pub(crate) fn parse(&mut self, term_output: &[u8]) -> Result<(), TermError> {
        let lines: Vec<_> = term_output.split(|&ch| ch == b'\n').collect();
        let line_count = lines.len();

        for (i, line) in lines.into_iter().enumerate() {
            let line = if line.last().copied() == Some(b'\r') {
                &line[..line.len() - 1]
            } else {
                line
            };

            // We ignore everything before the last occurrence of `\r` as a stop-gap measure
            // that works reasonably well in some cases.
            let processed_line = line.rsplitn(2, |&ch| ch == b'\r').next().unwrap_or(&[]);
            self.parse_line(processed_line)?;

            if i + 1 < line_count {
                writeln!(self.writer).map_err(TermError::Io)?;
            }
        }
        Ok(())
    }

    fn parse_line(&mut self, term_output: &[u8]) -> Result<(), TermError> {
        const ANSI_ESC: u8 = 0x1b;
        const ANSI_BEL: u8 = 0x07;
        const ANSI_CSI: u8 = b'[';
        const ANSI_OCS: u8 = b']';

        let mut dirty_color_spec = false;

        let mut i = 0;
        let mut written_end = 0;
        while i < term_output.len() {
            if term_output[i] == ANSI_ESC {
                // Push the preceding "ordinary" bytes into the writer.
                self.write_ordinary_text(&term_output[written_end..i], &mut dirty_color_spec)?;

                i += 1;
                let next_byte = term_output
                    .get(i)
                    .copied()
                    .ok_or(TermError::UnfinishedSequence)?;
                if next_byte == ANSI_CSI {
                    i += 1;
                    let csi = Csi::parse(&term_output[i..])?;
                    let prev_color_spec = self.color_spec.clone();
                    csi.update_color_spec(&mut self.color_spec)?;
                    dirty_color_spec = dirty_color_spec || prev_color_spec != self.color_spec;
                    i += csi.len;
                } else if next_byte == ANSI_OCS {
                    // Operating system command. Skip all chars until BEL (\u{7}) or ST (\u{1b}\).
                    while i < term_output.len()
                        && term_output[i] != ANSI_BEL
                        && term_output[i] != ANSI_ESC
                    {
                        i += 1;
                    }

                    if i == term_output.len() {
                        return Err(TermError::UnfinishedSequence);
                    }
                    if term_output[i] == ANSI_ESC {
                        i += 1;
                        if i == term_output.len() {
                            return Err(TermError::UnfinishedSequence);
                        }
                        if term_output[i] != b'\\' {
                            return Err(TermError::UnrecognizedSequence(term_output[i]));
                        }
                    }
                    i += 1;
                } else {
                    return Err(TermError::UnrecognizedSequence(next_byte));
                }
                written_end = i; // skip the escape sequence
            } else {
                // Ordinary char.
                i += 1;
            }
        }

        // We write the terminal color spec even if the text is empty.
        if dirty_color_spec {
            self.writer
                .set_color(&self.color_spec)
                .map_err(TermError::Io)?;
        }
        self.writer
            .write_all(&term_output[written_end..i])
            .map_err(TermError::Io)
    }

    fn write_ordinary_text(
        &mut self,
        text: &[u8],
        dirty_color_spec: &mut bool,
    ) -> Result<(), TermError> {
        if text.is_empty() {
            Ok(())
        } else {
            if *dirty_color_spec {
                *dirty_color_spec = false;
                self.writer
                    .set_color(&self.color_spec)
                    .map_err(TermError::Io)?;
            }
            self.writer.write_all(text).map_err(TermError::Io)
        }
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

    fn update_color_spec(self, spec: &mut ColorSpec) -> Result<(), TermError> {
        if self.final_byte != b'm' {
            return Ok(());
        }

        let mut params = self.parameters.split(|&byte| byte == b';').peekable();
        if params.peek().is_none() {
            *spec = ColorSpec::new(); // reset
        }
        while params.peek().is_some() {
            Self::process_param(spec, &mut params)?;
        }
        Ok(())
    }

    fn process_param(
        spec: &mut ColorSpec,
        mut params: impl Iterator<Item = &'a [u8]>,
    ) -> Result<(), TermError> {
        let param = params.next().unwrap();
        if let Some(fg_color) = Self::parse_simple_fg_color(param) {
            spec.set_fg(Some(fg_color));
        } else if let Some(bg_color) = Self::parse_simple_bg_color(param) {
            spec.set_bg(Some(bg_color));
        } else {
            match param {
                b"" | b"0" => {
                    *spec = ColorSpec::new();
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

                b"22" => {
                    spec.set_bold(false).set_dimmed(false);
                }
                b"23" => {
                    spec.set_italic(false);
                }
                b"24" => {
                    spec.set_underline(false);
                }

                // Compound foreground color spec
                b"38" => {
                    let color = Self::read_color(params)?;
                    spec.set_fg(Some(color));
                }
                b"39" => {
                    spec.set_fg(None);
                }
                // Compound background color spec
                b"48" => {
                    let color = Self::read_color(params)?;
                    spec.set_bg(Some(color));
                }
                b"49" => {
                    spec.set_bg(None);
                }

                _ => { /* Do nothing */ }
            }
        }
        Ok(())
    }

    fn parse_simple_fg_color(param: &[u8]) -> Option<Color> {
        Some(match param {
            b"30" => Color::Black,
            b"31" => Color::Red,
            b"32" => Color::Green,
            b"33" => Color::Yellow,
            b"34" => Color::Blue,
            b"35" => Color::Magenta,
            b"36" => Color::Cyan,
            b"37" => Color::White,

            b"90" => Color::Ansi256(8),
            b"91" => Color::Ansi256(9),
            b"92" => Color::Ansi256(10),
            b"93" => Color::Ansi256(11),
            b"94" => Color::Ansi256(12),
            b"95" => Color::Ansi256(13),
            b"96" => Color::Ansi256(14),
            b"97" => Color::Ansi256(15),

            _ => return None,
        })
    }

    fn parse_simple_bg_color(param: &[u8]) -> Option<Color> {
        Some(match param {
            b"40" => Color::Black,
            b"41" => Color::Red,
            b"42" => Color::Green,
            b"43" => Color::Yellow,
            b"44" => Color::Blue,
            b"45" => Color::Magenta,
            b"46" => Color::Cyan,
            b"47" => Color::White,

            b"100" => Color::Ansi256(8),
            b"101" => Color::Ansi256(9),
            b"102" => Color::Ansi256(10),
            b"103" => Color::Ansi256(11),
            b"104" => Color::Ansi256(12),
            b"105" => Color::Ansi256(13),
            b"106" => Color::Ansi256(14),
            b"107" => Color::Ansi256(15),

            _ => return None,
        })
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
