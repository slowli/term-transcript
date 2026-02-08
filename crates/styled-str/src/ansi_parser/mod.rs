//! Parser for terminal output that converts it to a sequence of instructions to
//! a writer implementing `WriteColor`.

use core::str;
use std::num::NonZeroUsize;

use anstyle::{Ansi256Color, AnsiColor, Color, Effects, RgbColor, Style};

pub use self::errors::AnsiError;
use crate::{StyledSpan, StyledString, utils::normalize_style};

mod errors;
#[cfg(test)]
mod tests;

const ANSI_ESC: u8 = 0x1b;
const ANSI_BEL: u8 = 0x07;
const ANSI_CSI: u8 = b'[';
const ANSI_OCS: u8 = b']';

/// Parses terminal output.
#[derive(Debug, Default)]
pub(crate) struct AnsiParser {
    output: StyledString,
    current_style: Style,
}

impl AnsiParser {
    pub(crate) fn parse(ansi_bytes: &[u8]) -> Result<StyledString, AnsiError> {
        let mut this = Self::default();
        this.process(ansi_bytes)?;
        Ok(this.into_styled())
    }

    /// Handles an operating system command. Skip all chars until BEL (\u{7}) or ST (\u{1b}).
    fn skip_ocs(term_output: &[u8], i: &mut usize) -> Result<(), AnsiError> {
        while *i < term_output.len() && term_output[*i] != ANSI_BEL && term_output[*i] != ANSI_ESC {
            *i += 1;
        }

        if *i == term_output.len() {
            return Err(AnsiError::UnfinishedSequence);
        }
        if term_output[*i] == ANSI_ESC {
            *i += 1;
            if *i == term_output.len() {
                return Err(AnsiError::UnfinishedSequence);
            }
            if term_output[*i] != b'\\' {
                return Err(AnsiError::UnrecognizedSequence(term_output[*i]));
            }
        }
        *i += 1;
        Ok(())
    }

    /// Checks whether the given chunk of text has any non-escaped parts.
    fn has_plaintext(bytes: &[u8]) -> Result<bool, AnsiError> {
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != ANSI_ESC {
                return Ok(true);
            }

            i += 1;
            let next_byte = bytes.get(i).copied().ok_or(AnsiError::UnfinishedSequence)?;
            if next_byte == ANSI_CSI {
                i += 1;
                let csi = Csi::parse(&bytes[i..])?;
                i += csi.len;
            } else if next_byte == ANSI_OCS {
                Self::skip_ocs(bytes, &mut i)?;
            } else {
                return Err(AnsiError::UnrecognizedSequence(next_byte));
            }
        }
        Ok(false)
    }

    fn write_text(&mut self, text: &str) {
        if let Some(len) = NonZeroUsize::new(text.len()) {
            self.output.text.push_str(text);
            self.output.spans.push(StyledSpan {
                style: normalize_style(self.current_style),
                len,
            });
        }
    }

    fn process(&mut self, ansi_bytes: &[u8]) -> Result<(), AnsiError> {
        let lines: Vec<_> = ansi_bytes.split(|&ch| ch == b'\n').collect();
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
                self.write_text("\n");
            }
        }
        Ok(())
    }

    fn into_styled(self) -> StyledString {
        self.output.shrink()
    }

    fn parse_line(&mut self, term_output: &[u8]) -> Result<(), AnsiError> {
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
                    .ok_or(AnsiError::UnfinishedSequence)?;
                if next_byte == ANSI_CSI {
                    i += 1;
                    let csi = Csi::parse(&term_output[i..])?;
                    let prev_color_spec = self.current_style;
                    csi.update_color_spec(&mut self.current_style)?;
                    dirty_color_spec = dirty_color_spec || prev_color_spec != self.current_style;
                    i += csi.len;
                } else if next_byte == ANSI_OCS {
                    Self::skip_ocs(term_output, &mut i)?;
                } else {
                    return Err(AnsiError::UnrecognizedSequence(next_byte));
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
        let remaining_text = str::from_utf8(&term_output[written_end..i])?;
        self.write_text(remaining_text);
        Ok(())
    }

    fn write_ordinary_text(
        &mut self,
        text: &[u8],
        dirty_color_spec: &mut bool,
    ) -> Result<(), AnsiError> {
        if text.is_empty() {
            Ok(())
        } else {
            let text = str::from_utf8(text)?;
            *dirty_color_spec = false;
            self.write_text(text);
            Ok(())
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
    fn parse(buffer: &'a [u8]) -> Result<Self, AnsiError> {
        let intermediates_start = buffer
            .iter()
            .position(|byte| !(0x30..=0x3f).contains(byte))
            .ok_or(AnsiError::UnfinishedSequence)?;

        let final_byte_pos = buffer[intermediates_start..]
            .iter()
            .position(|byte| !(0x20..=0x2f).contains(byte))
            .ok_or(AnsiError::UnfinishedSequence)?;
        let final_byte_pos = intermediates_start + final_byte_pos;

        let final_byte = buffer[final_byte_pos];
        if (0x40..=0x7e).contains(&final_byte) {
            Ok(Self {
                parameters: &buffer[..intermediates_start],
                final_byte,
                len: final_byte_pos + 1,
            })
        } else {
            Err(AnsiError::InvalidSgrFinalByte(final_byte))
        }
    }

    fn update_color_spec(self, spec: &mut Style) -> Result<(), AnsiError> {
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
    ) -> Result<(), AnsiError> {
        let param = params.next().unwrap();
        if let Some(fg_color) = Self::parse_simple_fg_color(param) {
            *style = style.fg_color(Some(fg_color.into()));
        } else if let Some(bg_color) = Self::parse_simple_bg_color(param) {
            *style = style.bg_color(Some(bg_color.into()));
        } else {
            match param {
                b"" | b"0" => {
                    *style = Style::default();
                }
                b"1" => {
                    *style = style.bold();
                }
                b"2" => {
                    *style = style.dimmed();
                }
                b"3" => {
                    *style = style.italic();
                }
                b"4" => {
                    *style = style.underline();
                }
                b"5" | b"6" => {
                    *style = style.blink();
                }
                b"7" => {
                    *style = style.invert();
                }
                b"8" => {
                    *style = style.hidden();
                }
                b"9" => {
                    *style = style.strikethrough();
                }

                b"22" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::BOLD).remove(Effects::DIMMED));
                }
                b"23" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::ITALIC));
                }
                b"24" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::UNDERLINE));
                }
                b"25" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::BLINK));
                }
                b"27" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::INVERT));
                }
                b"28" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::HIDDEN));
                }
                b"29" => {
                    let effects = style.get_effects();
                    *style = style.effects(effects.remove(Effects::STRIKETHROUGH));
                }

                // Compound foreground color spec
                b"38" => {
                    let color = Self::read_color(params)?;
                    *style = style.fg_color(Some(color));
                }
                b"39" => {
                    *style = style.fg_color(None);
                }
                // Compound background color spec
                b"48" => {
                    let color = Self::read_color(params)?;
                    *style = style.bg_color(Some(color));
                }
                b"49" => {
                    *style = style.bg_color(None);
                }

                _ => { /* Do nothing */ }
            }
        }
        Ok(())
    }

    fn parse_simple_fg_color(param: &[u8]) -> Option<AnsiColor> {
        Some(match param {
            b"30" => AnsiColor::Black,
            b"31" => AnsiColor::Red,
            b"32" => AnsiColor::Green,
            b"33" => AnsiColor::Yellow,
            b"34" => AnsiColor::Blue,
            b"35" => AnsiColor::Magenta,
            b"36" => AnsiColor::Cyan,
            b"37" => AnsiColor::White,

            b"90" => AnsiColor::BrightBlack,
            b"91" => AnsiColor::BrightRed,
            b"92" => AnsiColor::BrightGreen,
            b"93" => AnsiColor::BrightYellow,
            b"94" => AnsiColor::BrightBlue,
            b"95" => AnsiColor::BrightMagenta,
            b"96" => AnsiColor::BrightCyan,
            b"97" => AnsiColor::BrightWhite,

            _ => return None,
        })
    }

    fn parse_simple_bg_color(param: &[u8]) -> Option<AnsiColor> {
        Some(match param {
            b"40" => AnsiColor::Black,
            b"41" => AnsiColor::Red,
            b"42" => AnsiColor::Green,
            b"43" => AnsiColor::Yellow,
            b"44" => AnsiColor::Blue,
            b"45" => AnsiColor::Magenta,
            b"46" => AnsiColor::Cyan,
            b"47" => AnsiColor::White,

            b"100" => AnsiColor::BrightBlack,
            b"101" => AnsiColor::BrightRed,
            b"102" => AnsiColor::BrightGreen,
            b"103" => AnsiColor::BrightYellow,
            b"104" => AnsiColor::BrightBlue,
            b"105" => AnsiColor::BrightMagenta,
            b"106" => AnsiColor::BrightCyan,
            b"107" => AnsiColor::BrightWhite,

            _ => return None,
        })
    }

    fn read_color(mut params: impl Iterator<Item = &'a [u8]>) -> Result<Color, AnsiError> {
        let color_type = params.next().ok_or(AnsiError::UnfinishedColor)?;
        match color_type {
            b"5" => {
                let index = params.next().ok_or(AnsiError::UnfinishedColor)?;
                Self::parse_color_index(index).map(|index| Color::Ansi256(Ansi256Color(index)))
            }
            b"2" => {
                let r = params.next().ok_or(AnsiError::UnfinishedColor)?;
                let g = params.next().ok_or(AnsiError::UnfinishedColor)?;
                let b = params.next().ok_or(AnsiError::UnfinishedColor)?;

                let r = Self::parse_color_index(r)?;
                let g = Self::parse_color_index(g)?;
                let b = Self::parse_color_index(b)?;
                Ok(Color::Rgb(RgbColor(r, g, b)))
            }
            _ => {
                let color_type = String::from_utf8_lossy(color_type).into_owned();
                Err(AnsiError::InvalidColorType(color_type))
            }
        }
    }

    fn parse_color_index(param: &[u8]) -> Result<u8, AnsiError> {
        if param.is_empty() {
            // As per ANSI standards, empty params are treated as number 0.
            return Ok(0);
        }

        let param = unsafe {
            // SAFETY: safe by construction; we've checked range of bytes in params
            // when creating a `Csi` instance.
            str::from_utf8_unchecked(param)
        };
        param.parse().map_err(AnsiError::InvalidColorIndex)
    }
}
