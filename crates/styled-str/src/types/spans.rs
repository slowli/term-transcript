//! Span types.

use core::num::NonZeroUsize;

use anstyle::Style;
use compile_fmt::compile_panic;

/// Continuous span of styled text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StyledSpan {
    /// Style applied to the text.
    pub(crate) style: Style,
    /// Starting position of the span in text.
    pub(crate) start: usize,
    /// Length of text in bytes.
    pub(crate) len: NonZeroUsize,
}

impl StyledSpan {
    pub(crate) const DUMMY: Self = Self {
        style: Style::new(),
        start: 0,
        len: NonZeroUsize::new(1).unwrap(),
    };

    pub(crate) const fn end(&self) -> usize {
        self.start + self.len.get()
    }

    pub(crate) const fn extend_len(&mut self, add: usize) {
        self.len = self.len.checked_add(add).expect("length overflow");
    }

    pub(crate) const fn shrink_len(&mut self, sub: usize) {
        let new_len = self.len.get().checked_sub(sub).expect("length underflow");
        self.len = NonZeroUsize::new(new_len).expect("length underflow");
    }
}

/// Text with a uniform [`Style`] attached to it. Returned by the [`StyledStr::spans()`](crate::StyledStr::spans()) iterator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpanStr<'a> {
    /// Unstyled text.
    pub text: &'a str,
    /// Style applied to the text.
    pub style: Style,
}

impl<'a> SpanStr<'a> {
    /// Creates a string spanned with the specified style.
    ///
    /// # Panics
    ///
    /// Panics if `text` contains `\x1b` escapes.
    pub const fn new(text: &'a str, style: Style) -> Self {
        let text_bytes = text.as_bytes();
        let mut pos = 0;
        while pos < text_bytes.len() {
            if text_bytes[pos] == 0x1b {
                compile_panic!(
                    "text contains \\x1b escape, first at position ",
                    pos => compile_fmt::fmt::<usize>()
                );
            }
            pos += 1;
        }
        Self { text, style }
    }

    /// Creates a string with the default style.
    ///
    /// # Panics
    ///
    /// Panics if `text` contains `\x1b` escapes.
    pub const fn plain(text: &'a str) -> Self {
        Self::new(text, Style::new())
    }
}
