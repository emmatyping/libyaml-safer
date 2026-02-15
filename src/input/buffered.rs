use arraydeque::ArrayDeque;

use crate::char_traits;
use crate::input::Input;

/// The size of the internal buffer.
///
/// Must be at least 8 (for escape sequences). Most lookaheads are 4 or fewer.
const BUFFER_LEN: usize = 16;

/// A parser input backed by an `Iterator<Item = char>` with a fixed-size buffer.
///
/// This is suitable for streaming inputs where the full content is not available
/// up front. Characters are pulled from the iterator into an [`ArrayDeque`] as
/// needed by `lookahead()`.
pub struct BufferedInput<T: Iterator<Item = char>> {
    input: T,
    buffer: ArrayDeque<char, BUFFER_LEN>,
}

impl<T: Iterator<Item = char>> BufferedInput<T> {
    /// Create a new [`BufferedInput`] wrapping the given character iterator.
    pub fn new(input: T) -> Self {
        Self {
            input,
            buffer: ArrayDeque::default(),
        }
    }
}

impl<T: Iterator<Item = char>> Input for BufferedInput<T> {
    #[inline]
    fn lookahead(&mut self, count: usize) {
        if self.buffer.len() >= count {
            return;
        }
        for _ in 0..(count - self.buffer.len()) {
            let ch = self.input.next().unwrap_or('\0');
            self.buffer.push_back(ch).expect("buffer overflow");
        }
    }

    #[inline]
    fn buflen(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    fn peek(&self) -> char {
        self.buffer.get(0).copied().unwrap_or('\0')
    }

    #[inline]
    fn peek_nth(&self, n: usize) -> char {
        self.buffer.get(n).copied().unwrap_or('\0')
    }

    #[inline]
    fn skip(&mut self) {
        self.buffer.pop_front();
    }

    #[inline]
    fn skip_n(&mut self, count: usize) {
        self.buffer.drain(0..count);
    }

    #[inline]
    fn next_is_z(&self) -> bool {
        self.buffer.is_empty() || char_traits::is_z(self.buffer[0])
    }

    #[inline]
    fn next_is_break(&self) -> bool {
        !self.buffer.is_empty() && char_traits::is_break(self.buffer[0])
    }

    #[inline]
    fn next_is_breakz(&self) -> bool {
        self.buffer.is_empty() || char_traits::is_breakz(self.buffer[0])
    }

    #[inline]
    fn next_is_blank(&self) -> bool {
        !self.buffer.is_empty() && char_traits::is_blank(self.buffer[0])
    }

    #[inline]
    fn next_is_blank_or_breakz(&self) -> bool {
        self.buffer.is_empty() || char_traits::is_blank_or_breakz(self.buffer[0])
    }
}
