//! General-purpose utils.

use core::{fmt, ops, str};

/// Version of `try!` / `?` that can be used in const fns.
macro_rules! const_try {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(err) => return Err(err),
        }
    };
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
