//! `Palette` and other color-related types.

use std::{error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::utils::RgbColor;

/// Palette of [16 standard terminal colors][colors] (8 ordinary colors + 8 intense variations).
///
/// [colors]: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Palette {
    /// Ordinary colors.
    pub colors: TermColors,
    /// Intense colors.
    pub intense_colors: TermColors,
}

/// Returns the palette specified by [`NamedPalette::Gjm8`].
impl Default for Palette {
    fn default() -> Self {
        Self::gjm8()
    }
}

impl Palette {
    const fn dracula() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x28, 0x29, 0x36),
                red: RgbColor(0xea, 0x51, 0xb2),
                green: RgbColor(0xeb, 0xff, 0x87),
                yellow: RgbColor(0x00, 0xf7, 0x69),
                blue: RgbColor(0x62, 0xd6, 0xe8),
                magenta: RgbColor(0xb4, 0x5b, 0xcf),
                cyan: RgbColor(0xa1, 0xef, 0xe4),
                white: RgbColor(0xe9, 0xe9, 0xf4),
            },
            intense_colors: TermColors {
                black: RgbColor(0x62, 0x64, 0x83),
                red: RgbColor(0xb4, 0x5b, 0xcf),
                green: RgbColor(0x3a, 0x3c, 0x4e),
                yellow: RgbColor(0x4d, 0x4f, 0x68),
                blue: RgbColor(0x62, 0xd6, 0xe8),
                magenta: RgbColor(0xf1, 0xf2, 0xf8),
                cyan: RgbColor(0x00, 0xf7, 0x69),
                white: RgbColor(0xf7, 0xf7, 0xfb),
            },
        }
    }

    const fn powershell() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x0c, 0x0c, 0x0c),
                red: RgbColor(0xc5, 0x0f, 0x1f),
                green: RgbColor(0x13, 0xa1, 0x0e),
                yellow: RgbColor(0xc1, 0x9c, 0x00),
                blue: RgbColor(0x00, 0x37, 0xda),
                magenta: RgbColor(0x88, 0x17, 0x98),
                cyan: RgbColor(0x3a, 0x96, 0xdd),
                white: RgbColor(0xcc, 0xcc, 0xcc),
            },
            intense_colors: TermColors {
                black: RgbColor(0x76, 0x76, 0x76),
                red: RgbColor(0xe7, 0x48, 0x56),
                green: RgbColor(0x16, 0xc6, 0x0c),
                yellow: RgbColor(0xf9, 0xf1, 0xa5),
                blue: RgbColor(0x3b, 0x78, 0xff),
                magenta: RgbColor(0xb4, 0x00, 0x9e),
                cyan: RgbColor(0x61, 0xd6, 0xd6),
                white: RgbColor(0xf2, 0xf2, 0xf2),
            },
        }
    }

    const fn xterm() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0, 0, 0),
                red: RgbColor(0xcd, 0, 0),
                green: RgbColor(0, 0xcd, 0),
                yellow: RgbColor(0xcd, 0xcd, 0),
                blue: RgbColor(0, 0, 0xee),
                magenta: RgbColor(0xcd, 0, 0xcd),
                cyan: RgbColor(0, 0xcd, 0xcd),
                white: RgbColor(0xe5, 0xe5, 0xe5),
            },
            intense_colors: TermColors {
                black: RgbColor(0x7f, 0x7f, 0x7f),
                red: RgbColor(0xff, 0, 0),
                green: RgbColor(0, 0xff, 0),
                yellow: RgbColor(0xff, 0xff, 0),
                blue: RgbColor(0x5c, 0x5c, 0xff),
                magenta: RgbColor(0xff, 0, 0xff),
                cyan: RgbColor(0, 0xff, 0xff),
                white: RgbColor(0xff, 0xff, 0xff),
            },
        }
    }

    const fn ubuntu() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x01, 0x01, 0x01),
                red: RgbColor(0xde, 0x38, 0x2b),
                green: RgbColor(0x38, 0xb5, 0x4a),
                yellow: RgbColor(0xff, 0xc7, 0x06),
                blue: RgbColor(0, 0x6f, 0xb8),
                magenta: RgbColor(0x76, 0x26, 0x71),
                cyan: RgbColor(0x2c, 0xb5, 0xe9),
                white: RgbColor(0xcc, 0xcc, 0xcc),
            },
            intense_colors: TermColors {
                black: RgbColor(0x80, 0x80, 0x80),
                red: RgbColor(0xff, 0, 0),
                green: RgbColor(0, 0xff, 0),
                yellow: RgbColor(0xff, 0xff, 0),
                blue: RgbColor(0, 0, 0xff),
                magenta: RgbColor(0xff, 0, 0xff),
                cyan: RgbColor(0, 0xff, 0xff),
                white: RgbColor(0xff, 0xff, 0xff),
            },
        }
    }

    const fn gjm8() -> Self {
        Self {
            colors: TermColors {
                black: RgbColor(0x1c, 0x1c, 0x1c),
                red: RgbColor(0xff, 0x00, 0x5b),
                green: RgbColor(0xce, 0xe3, 0x18),
                yellow: RgbColor(0xff, 0xe7, 0x55),
                blue: RgbColor(0x04, 0x8a, 0xc7),
                magenta: RgbColor(0x83, 0x3c, 0x9f),
                cyan: RgbColor(0x0a, 0xc1, 0xcd),
                white: RgbColor(0xe5, 0xe5, 0xe5),
            },
            intense_colors: TermColors {
                black: RgbColor(0x66, 0x66, 0x66),
                red: RgbColor(0xff, 0x00, 0xa0),
                green: RgbColor(0xcc, 0xff, 0x00),
                yellow: RgbColor(0xff, 0x9f, 0x00),
                blue: RgbColor(0x48, 0xc6, 0xff),
                magenta: RgbColor(0xbe, 0x67, 0xe1),
                cyan: RgbColor(0x63, 0xe7, 0xf0),
                white: RgbColor(0xf3, 0xf3, 0xf3),
            },
        }
    }
}

/// Values of [8 base terminal colors][colors].
///
/// [colors]: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TermColors {
    /// Black color.
    pub black: RgbColor,
    /// Red color.
    pub red: RgbColor,
    /// Green color.
    pub green: RgbColor,
    /// Yellow color.
    pub yellow: RgbColor,
    /// Blue color.
    pub blue: RgbColor,
    /// Magenta color.
    pub magenta: RgbColor,
    /// Cyan color.
    pub cyan: RgbColor,
    /// White color.
    pub white: RgbColor,
}

impl Serialize for RgbColor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{self:x}"))
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        #[derive(Debug)]
        struct ColorVisitor;

        impl de::Visitor<'_> for ColorVisitor {
            type Value = RgbColor;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("hex color, such as #fed or #a757ff")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

/// Named [`Palette`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NamedPalette {
    /// Dracula color scheme. This is the [`Default`] value.
    Dracula,
    /// PowerShell 6 / Windows 10 console color scheme.
    PowerShell,
    /// `xterm` color scheme.
    Xterm,
    /// Ubuntu terminal color scheme.
    Ubuntu,
    /// [gjm8 color scheme](https://terminal.sexy/).
    #[default]
    Gjm8,
}

impl From<NamedPalette> for Palette {
    fn from(value: NamedPalette) -> Self {
        match value {
            NamedPalette::Dracula => Self::dracula(),
            NamedPalette::PowerShell => Self::powershell(),
            NamedPalette::Xterm => Self::xterm(),
            NamedPalette::Ubuntu => Self::ubuntu(),
            NamedPalette::Gjm8 => Self::gjm8(),
        }
    }
}

impl FromStr for NamedPalette {
    type Err = NamedPaletteParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dracula" => Ok(Self::Dracula),
            "powershell" => Ok(Self::PowerShell),
            "xterm" => Ok(Self::Xterm),
            "ubuntu" => Ok(Self::Ubuntu),
            "gjm8" => Ok(Self::Gjm8),
            _ => Err(NamedPaletteParseError(())),
        }
    }
}

/// Errors that can occur when [parsing](FromStr) [`NamedPalette`] from a string.
#[derive(Debug)]
pub struct NamedPaletteParseError(());

impl fmt::Display for NamedPaletteParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "Invalid palette name; allowed names are `dracula`, `powershell`, `xterm`, \
             `ubuntu` and `gjm8`",
        )
    }
}

impl error::Error for NamedPaletteParseError {}
