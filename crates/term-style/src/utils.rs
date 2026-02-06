//! General-purpose utils.

use core::{fmt, ops, str};

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};

/// Version of `try!` / `?` that can be used in const fns.
macro_rules! const_try {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(err) => return Err(err),
        }
    };
}

const fn color_cube_color(index: u8) -> u8 {
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

const fn normalize_color(color: Color) -> Color {
    const STD_COLORS: [AnsiColor; 16] = [
        AnsiColor::Black,
        AnsiColor::Red,
        AnsiColor::Green,
        AnsiColor::Yellow,
        AnsiColor::Blue,
        AnsiColor::Magenta,
        AnsiColor::Cyan,
        AnsiColor::White,
        AnsiColor::BrightBlack,
        AnsiColor::BrightRed,
        AnsiColor::BrightGreen,
        AnsiColor::BrightYellow,
        AnsiColor::BrightBlue,
        AnsiColor::BrightMagenta,
        AnsiColor::BrightCyan,
        AnsiColor::BrightWhite,
    ];

    if let Color::Ansi256(Ansi256Color(index)) = color {
        match index {
            0..=15 => Color::Ansi(STD_COLORS[index as usize]),

            16..=231 => {
                let index = index - 16;
                let r = color_cube_color(index / 36);
                let g = color_cube_color((index / 6) % 6);
                let b = color_cube_color(index % 6);
                Color::Rgb(RgbColor(r, g, b))
            }

            _ => {
                let gray = 10 * (index - 232) + 8;
                Color::Rgb(RgbColor(gray, gray, gray))
            }
        }
    } else {
        color
    }
}

pub(crate) const fn normalize_style(mut style: Style) -> Style {
    if let Some(color) = style.get_fg_color() {
        style = style.fg_color(Some(normalize_color(color)));
    }
    if let Some(color) = style.get_bg_color() {
        style = style.bg_color(Some(normalize_color(color)));
    }
    style
}

pub(crate) const fn is_same_style(lhs: &Style, rhs: &Style) -> bool {
    let lhs_effects = lhs.get_effects();
    let rhs_effects = rhs.get_effects();
    if !lhs_effects.contains(rhs_effects) || !rhs_effects.contains(lhs_effects) {
        // Ideally, we'd want to compare effects directly, but this isn't constant-time
        return false;
    }

    is_same_color_opt(lhs.get_fg_color(), rhs.get_fg_color())
        && is_same_color_opt(lhs.get_bg_color(), rhs.get_bg_color())
}

const fn is_same_color_opt(lhs: Option<Color>, rhs: Option<Color>) -> bool {
    match (lhs, rhs) {
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
        (Some(lhs), Some(rhs)) => is_same_color(lhs, rhs),
    }
}

const fn is_same_color(lhs: Color, rhs: Color) -> bool {
    match (lhs, rhs) {
        (Color::Ansi(lhs), Color::Ansi(rhs)) => lhs as u8 == rhs as u8,
        (Color::Ansi256(Ansi256Color(lhs)), Color::Ansi256(Ansi256Color(rhs))) => lhs == rhs,
        (Color::Rgb(RgbColor(rl, gl, bl)), Color::Rgb(RgbColor(rr, gr, br))) => {
            rl == rr && gl == gr && bl == br
        }
        _ => false,
    }
}

const UTF8_CONTINUATION_MASK: u8 = 0b1100_0000;
const UTF8_CONTINUATION_MARKER: u8 = 0b1000_0000;

const fn ceil_char_boundary(bytes: &[u8], mut pos: usize) -> usize {
    assert!(pos <= bytes.len());

    while pos < bytes.len() && bytes[pos] & UTF8_CONTINUATION_MASK == UTF8_CONTINUATION_MARKER {
        pos += 1;
    }
    pos
}

const fn floor_char_boundary(bytes: &[u8], mut pos: usize) -> usize {
    assert!(pos <= bytes.len());

    while pos > 0 && bytes[pos] & UTF8_CONTINUATION_MASK == UTF8_CONTINUATION_MARKER {
        pos -= 1;
    }
    pos
}

#[derive(Debug)]
pub(crate) struct PushError;

/// Bounded-capacity stack with `const fn` operations.
pub(crate) struct Stack<T, const N: usize> {
    inner: [T; N],
    len: usize,
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for Stack<T, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), formatter)
    }
}

impl<T: Copy, const N: usize> Stack<T, N> {
    pub(crate) const fn new(filler: T) -> Self {
        Self {
            inner: [filler; N],
            len: 0,
        }
    }

    pub(crate) const fn push(&mut self, item: T) -> Result<(), PushError> {
        if self.len == N {
            Err(PushError)
        } else {
            self.inner[self.len] = item;
            self.len += 1;
            Ok(())
        }
    }

    pub(crate) const fn last_mut(&mut self) -> Option<&mut T> {
        if self.len == 0 {
            None
        } else {
            Some(&mut self.inner[self.len - 1])
        }
    }
}

impl<T, const N: usize> Stack<T, N> {
    /// Returns the underlying slice of elements.
    pub(crate) const fn as_slice(&self) -> &[T] {
        let (start, _) = self.inner.split_at(self.len);
        start
    }
}

impl<T, const N: usize> AsRef<[T]> for Stack<T, N> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> ops::Deref for Stack<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: PartialEq, const N: usize> PartialEq for Stack<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

pub(crate) struct StackStr<const CAP: usize> {
    bytes: [u8; CAP],
    len: usize,
}

impl<const CAP: usize> fmt::Debug for StackStr<CAP> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.as_str(), formatter)
    }
}

impl<const CAP: usize> StackStr<CAP> {
    pub(crate) const fn new() -> Self {
        Self {
            bytes: [0; CAP],
            len: 0,
        }
    }

    pub(crate) const fn as_str(&self) -> &str {
        let (head, _) = self.bytes.split_at(self.len);
        unsafe { str::from_utf8_unchecked(head) }
    }

    /// The caller must guarantee that pushed bytes form a valid UTF-8 string.
    pub(crate) const fn push(&mut self, byte: u8) -> Result<(), PushError> {
        if self.len == CAP {
            return Err(PushError);
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct StrCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> StrCursor<'a> {
    pub(crate) const fn new(s: &'a str) -> Self {
        Self {
            bytes: s.as_bytes(),
            pos: 0,
        }
    }

    pub(crate) const fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) const fn current_byte(&self) -> u8 {
        assert!(self.pos < self.bytes.len());
        self.bytes[self.pos]
    }

    pub(crate) const fn range(&self, range: &ops::Range<usize>) -> &'a [u8] {
        assert!(range.end >= range.start);
        let (_, tail) = self.bytes.split_at(range.start);
        let (head, _) = tail.split_at(range.end - range.start);
        head
    }

    pub(crate) const fn is_eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    pub(crate) const fn advance_byte(&mut self) -> u8 {
        assert!(self.pos < self.bytes.len());
        let ch = self.bytes[self.pos];
        self.pos += 1;
        ch
    }

    pub(crate) const fn gobble(&mut self, needle: &str) -> bool {
        let needle = needle.as_bytes();
        let mut i = 0;
        while i < needle.len()
            && self.pos + i < self.bytes.len()
            && self.bytes[self.pos + i] == needle[i]
        {
            i += 1;
        }

        if i == needle.len() {
            // There's a match
            self.pos += needle.len();
            true
        } else {
            false
        }
    }

    pub(crate) const fn expand_to_char_boundaries(
        &self,
        range: ops::Range<usize>,
    ) -> ops::Range<usize> {
        assert!(range.start <= range.end);
        assert!(range.start < self.bytes.len());

        let start = floor_char_boundary(self.bytes, range.start);
        let end = ceil_char_boundary(self.bytes, range.end);
        start..end
    }
}
