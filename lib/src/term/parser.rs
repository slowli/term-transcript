//! Parser for terminal output that converts it to a sequence of instructions to
//! a writer implementing `WriteColor`.

use std::str;

use crate::{
    style::{Color, RgbColor, Style, WriteStyled},
    TermError,
};

const ANSI_ESC: u8 = 0x1b;
const ANSI_BEL: u8 = 0x07;
const ANSI_CSI: u8 = b'[';
const ANSI_OCS: u8 = b']';

/// Parses terminal output and issues corresponding commands to the `writer`.
#[derive(Debug)]
pub(crate) struct TermOutputParser<'a, W: ?Sized> {
    writer: &'a mut W,
    style: Style,
}

impl<'a, W: WriteStyled + ?Sized> TermOutputParser<'a, W> {
    pub(crate) fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            style: Style::default(),
        }
    }

    /// Handles an operating system command. Skip all chars until BEL (\u{7}) or ST (\u{1b}).
    fn skip_ocs(term_output: &[u8], i: &mut usize) -> Result<(), TermError> {
        while *i < term_output.len() && term_output[*i] != ANSI_BEL && term_output[*i] != ANSI_ESC {
            *i += 1;
        }

        if *i == term_output.len() {
            return Err(TermError::UnfinishedSequence);
        }
        if term_output[*i] == ANSI_ESC {
            *i += 1;
            if *i == term_output.len() {
                return Err(TermError::UnfinishedSequence);
            }
            if term_output[*i] != b'\\' {
                return Err(TermError::UnrecognizedSequence(term_output[*i]));
            }
        }
        *i += 1;
        Ok(())
    }

    /// Checks whether the given chunk of text has any non-escaped parts.
    fn has_plaintext(bytes: &[u8]) -> Result<bool, TermError> {
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != ANSI_ESC {
                return Ok(true);
            }

            i += 1;
            let next_byte = bytes.get(i).copied().ok_or(TermError::UnfinishedSequence)?;
            if next_byte == ANSI_CSI {
                i += 1;
                let csi = Csi::parse(&bytes[i..])?;
                i += csi.len;
            } else if next_byte == ANSI_OCS {
                Self::skip_ocs(bytes, &mut i)?;
            } else {
                return Err(TermError::UnrecognizedSequence(next_byte));
            }
        }
        Ok(false)
    }

    pub(crate) fn parse(&mut self, term_output: &[u8]) -> Result<(), TermError> {
        let lines: Vec<_> = term_output.split(|&ch| ch == b'\n').collect();
        let line_count = lines.len();

        for (i, line) in lines.into_iter().enumerate() {
            // Find the last occurrence of `\r` that has text output after it, and trim everything before it.
            // This works reasonably well in most cases.
            let chunks = line.rsplit(|&ch| ch == b'\r');
            let mut processed_len = 0;
            for chunk in chunks {
                processed_len += chunk.len() + 1;
                if Self::has_plaintext(chunk)? {
                    break;
                }
            }
            let start_pos = line.len().saturating_sub(processed_len);
            let processed_line = &line[start_pos..];

            self.parse_line(processed_line)?;

            if i + 1 < line_count {
                self.writer.write_text("\n").map_err(TermError::Io)?;
            }
        }
        Ok(())
    }

    fn parse_line(&mut self, term_output: &[u8]) -> Result<(), TermError> {
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
                    let prev_color_spec = self.style;
                    csi.update_color_spec(&mut self.style)?;
                    dirty_color_spec = dirty_color_spec || prev_color_spec != self.style;
                    i += csi.len;
                } else if next_byte == ANSI_OCS {
                    Self::skip_ocs(term_output, &mut i)?;
                } else {
                    return Err(TermError::UnrecognizedSequence(next_byte));
                }
                written_end = i; // skip the escape sequence
            } else if term_output[i] == b'\r' {
                self.write_ordinary_text(&term_output[written_end..i], &mut dirty_color_spec)?;
                i += 1;
                written_end = i; // skip writing '\r'
            } else {
                // Ordinary char.
                i += 1;
            }
        }

        // We write the terminal color spec even if the text is empty.
        if dirty_color_spec {
            self.writer
                .write_style(&self.style)
                .map_err(TermError::Io)?;
        }

        let remaining_text = str::from_utf8(&term_output[written_end..i])?;
        self.writer
            .write_text(remaining_text)
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
            let text = str::from_utf8(text)?;
            if *dirty_color_spec {
                *dirty_color_spec = false;
                self.writer
                    .write_style(&self.style)
                    .map_err(TermError::Io)?;
            }
            self.writer.write_text(text).map_err(TermError::Io)
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

    fn update_color_spec(self, spec: &mut Style) -> Result<(), TermError> {
        if self.final_byte != b'm' {
            return Ok(());
        }

        let mut params = self.parameters.split(|&byte| byte == b';').peekable();
        if params.peek().is_none() {
            *spec = Style::default(); // reset
        }
        while params.peek().is_some() {
            Self::process_param(spec, &mut params)?;
        }
        Ok(())
    }

    fn process_param(
        style: &mut Style,
        mut params: impl Iterator<Item = &'a [u8]>,
    ) -> Result<(), TermError> {
        let param = params.next().unwrap();
        if let Some(fg_color) = Self::parse_simple_fg_color(param) {
            style.fg = Some(fg_color);
        } else if let Some(bg_color) = Self::parse_simple_bg_color(param) {
            style.bg = Some(bg_color);
        } else {
            match param {
                b"" | b"0" => {
                    *style = Style::default();
                }
                b"1" => {
                    style.bold = true;
                }
                b"2" => {
                    style.dimmed = true;
                }
                b"3" => {
                    style.italic = true;
                }
                b"4" => {
                    style.underline = true;
                }

                b"22" => {
                    style.bold = false;
                    style.dimmed = false;
                }
                b"23" => {
                    style.italic = false;
                }
                b"24" => {
                    style.underline = false;
                }

                // Compound foreground color spec
                b"38" => {
                    let color = Self::read_color(params)?;
                    style.fg = Some(color);
                }
                b"39" => {
                    style.fg = None;
                }
                // Compound background color spec
                b"48" => {
                    let color = Self::read_color(params)?;
                    style.bg = Some(color);
                }
                b"49" => {
                    style.bg = None;
                }

                _ => { /* Do nothing */ }
            }
        }
        Ok(())
    }

    fn parse_simple_fg_color(param: &[u8]) -> Option<Color> {
        Some(match param {
            b"30" => Color::BLACK,
            b"31" => Color::RED,
            b"32" => Color::GREEN,
            b"33" => Color::YELLOW,
            b"34" => Color::BLUE,
            b"35" => Color::MAGENTA,
            b"36" => Color::CYAN,
            b"37" => Color::WHITE,

            b"90" => Color::INTENSE_BLACK,
            b"91" => Color::INTENSE_RED,
            b"92" => Color::INTENSE_GREEN,
            b"93" => Color::INTENSE_YELLOW,
            b"94" => Color::INTENSE_BLUE,
            b"95" => Color::INTENSE_MAGENTA,
            b"96" => Color::INTENSE_CYAN,
            b"97" => Color::INTENSE_WHITE,

            _ => return None,
        })
    }

    fn parse_simple_bg_color(param: &[u8]) -> Option<Color> {
        Some(match param {
            b"40" => Color::BLACK,
            b"41" => Color::RED,
            b"42" => Color::GREEN,
            b"43" => Color::YELLOW,
            b"44" => Color::BLUE,
            b"45" => Color::MAGENTA,
            b"46" => Color::CYAN,
            b"47" => Color::WHITE,

            b"100" => Color::INTENSE_BLACK,
            b"101" => Color::INTENSE_RED,
            b"102" => Color::INTENSE_GREEN,
            b"103" => Color::INTENSE_YELLOW,
            b"104" => Color::INTENSE_BLUE,
            b"105" => Color::INTENSE_MAGENTA,
            b"106" => Color::INTENSE_CYAN,
            b"107" => Color::INTENSE_WHITE,

            _ => return None,
        })
    }

    fn read_color(mut params: impl Iterator<Item = &'a [u8]>) -> Result<Color, TermError> {
        let color_type = params.next().ok_or(TermError::UnfinishedColor)?;
        match color_type {
            b"5" => {
                let index = params.next().ok_or(TermError::UnfinishedColor)?;
                Self::parse_color_index(index).map(Color::Index)
            }
            b"2" => {
                let r = params.next().ok_or(TermError::UnfinishedColor)?;
                let g = params.next().ok_or(TermError::UnfinishedColor)?;
                let b = params.next().ok_or(TermError::UnfinishedColor)?;

                let r = Self::parse_color_index(r)?;
                let g = Self::parse_color_index(g)?;
                let b = Self::parse_color_index(b)?;
                Ok(Color::Rgb(RgbColor(r, g, b)))
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
