//! `Style` and related types.

use std::{error::Error as StdError, fmt, io, num::ParseIntError, str::FromStr};

/// RGB color with 8-bit channels.
///
/// A color [can be parsed](FromStr) from a hex string like `#fed` or `#de382b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl fmt::LowerHex for RgbColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Errors that can occur when [parsing](FromStr) an [`RgbColor`] from a string.
#[derive(Debug)]
#[non_exhaustive]
pub enum RgbColorParseError {
    /// Color string contains non-ASCII chars.
    NotAscii,
    /// The color does not have a `#` prefix.
    NoHashPrefix,
    /// The color has incorrect string length (not 1 or 2 chars per color channel).
    /// The byte length of the string (including 1 char for the `#` prefix)
    /// is provided within this variant.
    IncorrectLen(usize),
    /// Error parsing color channel value.
    IncorrectDigit(ParseIntError),
}

impl fmt::Display for RgbColorParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAscii => formatter.write_str("color string contains non-ASCII chars"),
            Self::NoHashPrefix => formatter.write_str("missing '#' prefix"),
            Self::IncorrectLen(len) => write!(
                formatter,
                "unexpected byte length {len} of color string, expected 4 or 7"
            ),
            Self::IncorrectDigit(err) => write!(formatter, "error parsing hex digit: {err}"),
        }
    }
}

impl StdError for RgbColorParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::IncorrectDigit(err) => Some(err),
            _ => None,
        }
    }
}

impl FromStr for RgbColor {
    type Err = RgbColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.as_bytes()[0] != b'#' {
            Err(RgbColorParseError::NoHashPrefix)
        } else if s.len() == 4 {
            if !s.is_ascii() {
                return Err(RgbColorParseError::NotAscii);
            }

            let r = u8::from_str_radix(&s[1..2], 16).map_err(RgbColorParseError::IncorrectDigit)?;
            let g = u8::from_str_radix(&s[2..3], 16).map_err(RgbColorParseError::IncorrectDigit)?;
            let b = u8::from_str_radix(&s[3..], 16).map_err(RgbColorParseError::IncorrectDigit)?;
            Ok(Self(r * 17, g * 17, b * 17))
        } else if s.len() == 7 {
            if !s.is_ascii() {
                return Err(RgbColorParseError::NotAscii);
            }

            let r = u8::from_str_radix(&s[1..3], 16).map_err(RgbColorParseError::IncorrectDigit)?;
            let g = u8::from_str_radix(&s[3..5], 16).map_err(RgbColorParseError::IncorrectDigit)?;
            let b = u8::from_str_radix(&s[5..], 16).map_err(RgbColorParseError::IncorrectDigit)?;
            Ok(Self(r, g, b))
        } else {
            Err(RgbColorParseError::IncorrectLen(s.len()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "svg", derive(serde::Serialize))]
#[cfg_attr(feature = "svg", serde(untagged))]
pub(crate) enum Color {
    Index(u8),
    Rgb(RgbColor),
}

impl Color {
    pub(crate) const BLACK: Self = Self::Index(0);
    pub(crate) const RED: Self = Self::Index(1);
    pub(crate) const GREEN: Self = Self::Index(2);
    pub(crate) const YELLOW: Self = Self::Index(3);
    pub(crate) const BLUE: Self = Self::Index(4);
    pub(crate) const MAGENTA: Self = Self::Index(5);
    pub(crate) const CYAN: Self = Self::Index(6);
    pub(crate) const WHITE: Self = Self::Index(7);

    pub(crate) const INTENSE_BLACK: Self = Self::Index(8);
    pub(crate) const INTENSE_RED: Self = Self::Index(9);
    pub(crate) const INTENSE_GREEN: Self = Self::Index(10);
    pub(crate) const INTENSE_YELLOW: Self = Self::Index(11);
    pub(crate) const INTENSE_BLUE: Self = Self::Index(12);
    pub(crate) const INTENSE_MAGENTA: Self = Self::Index(13);
    pub(crate) const INTENSE_CYAN: Self = Self::Index(14);
    pub(crate) const INTENSE_WHITE: Self = Self::Index(15);

    fn index(value: u8) -> Self {
        debug_assert!(value < 16);
        Self::Index(value)
    }

    fn normalize(&mut self) {
        if let Self::Index(index) = *self {
            if index >= 16 {
                *self = Color::indexed_color(index);
            }
        }
    }

    pub(crate) fn indexed_color(index: u8) -> Self {
        match index {
            0..=15 => Self::index(index),

            16..=231 => {
                let index = index - 16;
                let r = Self::color_cube_color(index / 36);
                let g = Self::color_cube_color((index / 6) % 6);
                let b = Self::color_cube_color(index % 6);
                Self::Rgb(RgbColor(r, g, b))
            }

            _ => {
                let gray = 10 * (index - 232) + 8;
                Self::Rgb(RgbColor(gray, gray, gray))
            }
        }
    }

    fn color_cube_color(index: u8) -> u8 {
        match index {
            0 => 0,
            1 => 0x5f,
            2 => 0x87,
            3 => 0xaf,
            4 => 0xd7,
            5 => 0xff,
            _ => unreachable!(),
        }
    }
}

/// Serializable `ColorSpec` representation.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "svg", derive(serde::Serialize))]
#[allow(clippy::struct_excessive_bools)] // makes serialization simpler
pub(crate) struct Style {
    #[cfg_attr(feature = "svg", serde(skip_serializing_if = "Style::is_false"))]
    pub(crate) bold: bool,
    #[cfg_attr(feature = "svg", serde(skip_serializing_if = "Style::is_false"))]
    pub(crate) italic: bool,
    #[cfg_attr(feature = "svg", serde(skip_serializing_if = "Style::is_false"))]
    pub(crate) underline: bool,
    #[cfg_attr(feature = "svg", serde(skip_serializing_if = "Style::is_false"))]
    pub(crate) dimmed: bool,
    #[cfg_attr(feature = "svg", serde(skip_serializing_if = "Option::is_none"))]
    pub(crate) fg: Option<Color>,
    #[cfg_attr(feature = "svg", serde(skip_serializing_if = "Option::is_none"))]
    pub(crate) bg: Option<Color>,
}

impl Style {
    #[cfg(feature = "svg")]
    #[allow(clippy::trivially_copy_pass_by_ref)] // required by `serde`
    fn is_false(&val: &bool) -> bool {
        !val
    }

    pub(crate) fn is_none(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.underline
            && !self.dimmed
            && self.fg.is_none()
            && self.bg.is_none()
    }

    pub(crate) fn normalize(&mut self) {
        if let Some(color) = &mut self.fg {
            color.normalize();
        }
        if let Some(color) = &mut self.bg {
            color.normalize();
        }
    }

    fn write_to_io(&self, writer: &mut impl io::Write) -> io::Result<()> {
        // Reset the style first.
        write!(writer, "\u{1b}[0m")?;

        if self.bold {
            write!(writer, "\u{1b}[1m")?;
        }
        if self.dimmed {
            write!(writer, "\u{1b}[2m")?;
        }
        if self.italic {
            write!(writer, "\u{1b}[3m")?;
        }
        if self.underline {
            write!(writer, "\u{1b}[4m")?;
        }

        if let Some(fg) = &self.fg {
            fg.write_params_to_io(writer, false)?;
        }
        if let Some(bg) = &self.bg {
            bg.write_params_to_io(writer, true)?;
        }

        Ok(())
    }
}

impl Color {
    fn write_params_to_io(self, writer: &mut impl io::Write, is_bg: bool) -> io::Result<()> {
        match self {
            Self::Index(idx) if idx < 8 => {
                let offset = if is_bg { 40 } else { 30 };
                write!(writer, "\u{1b}[{}m", offset + idx)
            }
            Self::Index(idx) => {
                let prefix = if is_bg { 48 } else { 38 };
                write!(writer, "\u{1b}[{prefix};5;{idx}m")
            }
            Self::Rgb(RgbColor(r, g, b)) => {
                let prefix = if is_bg { 48 } else { 38 };
                write!(writer, "\u{1b}[{prefix};2;{r};{g};{b}m")
            }
        }
    }
}

/// Span of text with associated [`Style`].
#[cfg(any(feature = "svg", feature = "test"))]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "svg", derive(serde::Serialize))]
pub(crate) struct StyledSpan<T = String> {
    #[cfg_attr(feature = "svg", serde(flatten))]
    pub(crate) style: Style,
    pub(crate) text: T,
}

/// Writer similar to `io::Write`, but with separated writing of `Style`s.
pub(crate) trait WriteStyled {
    /// The style is completely reset on each call.
    fn write_style(&mut self, style: &Style) -> io::Result<()>;

    fn write_text(&mut self, text: &str) -> io::Result<()>;

    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        struct WriteWrapper<T>(io::Result<T>);

        impl<T: WriteStyled + ?Sized> fmt::Write for WriteWrapper<&mut T> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                if let Ok(writer) = &mut self.0 {
                    if let Err(err) = writer.write_text(s) {
                        self.0 = Err(err);
                    }
                }

                if self.0.is_err() {
                    Err(fmt::Error)
                } else {
                    Ok(())
                }
            }
        }

        let mut writer = WriteWrapper(Ok(self));
        fmt::Write::write_fmt(&mut writer, args).map_err(|_| writer.0.map(drop).unwrap_err())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.write_style(&Style::default())
    }
}

// No-op implementation
impl WriteStyled for io::Sink {
    fn write_style(&mut self, _style: &Style) -> io::Result<()> {
        Ok(())
    }

    fn write_text(&mut self, _text: &str) -> io::Result<()> {
        Ok(())
    }
}

/// `WriteStyled` implementation that writes styles as ANSI escape sequences.
#[derive(Debug)]
pub(crate) struct Ansi<W>(pub(crate) W);

impl<W: io::Write> WriteStyled for Ansi<W> {
    fn write_style(&mut self, style: &Style) -> io::Result<()> {
        style.write_to_io(&mut self.0)
    }

    fn write_text(&mut self, text: &str) -> io::Result<()> {
        self.0.write_all(text.as_bytes())
    }
}

impl WriteStyled for String {
    fn write_style(&mut self, _style: &Style) -> io::Result<()> {
        Ok(())
    }

    fn write_text(&mut self, text: &str) -> io::Result<()> {
        self.push_str(text);
        Ok(())
    }
}

impl<T: WriteStyled + ?Sized> WriteStyled for &mut T {
    fn write_style(&mut self, style: &Style) -> io::Result<()> {
        (**self).write_style(style)
    }

    fn write_text(&mut self, text: &str) -> io::Result<()> {
        (**self).write_text(text)
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use super::*;

    #[test]
    fn parsing_color() {
        let RgbColor(r, g, b) = "#fed".parse().unwrap();
        assert_eq!((r, g, b), (0xff, 0xee, 0xdd));
        let RgbColor(r, g, b) = "#c0ffee".parse().unwrap();
        assert_eq!((r, g, b), (0xc0, 0xff, 0xee));
    }

    #[test]
    fn errors_parsing_color() {
        let err = "123".parse::<RgbColor>().unwrap_err();
        assert_matches!(err, RgbColorParseError::NoHashPrefix);
        let err = "#12".parse::<RgbColor>().unwrap_err();
        assert_matches!(err, RgbColorParseError::IncorrectLen(3));
        let err = "#тэг".parse::<RgbColor>().unwrap_err();
        assert_matches!(err, RgbColorParseError::NotAscii);
        let err = "#coffee".parse::<RgbColor>().unwrap_err();
        assert_matches!(err, RgbColorParseError::IncorrectDigit(_));
    }
}
