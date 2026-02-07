//! General-purpose traits.

/// Pops a char from the end of a string.
pub trait PopChar {
    /// Performs popping.
    fn pop_char(&mut self) -> Option<char>;
}

impl PopChar for String {
    fn pop_char(&mut self) -> Option<char> {
        self.pop()
    }
}

impl PopChar for &str {
    fn pop_char(&mut self) -> Option<char> {
        let (pos, ch) = self.char_indices().next_back()?;
        *self = &self[..pos];
        Some(ch)
    }
}
