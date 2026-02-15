//! Input abstraction for the YAML scanner.
//!
//! The [`Input`] trait provides a character-level interface for the scanner,
//! abstracting over different input sources. Two implementations are provided:
//!
//! - [`StrInput`]: Optimized for `&str` inputs with ASCII fast-paths.
//! - [`BufferedInput`]: For streaming `Iterator<Item = char>` sources.

mod buffered;
mod owned_str;
mod str;

pub use self::buffered::BufferedInput;
pub use self::owned_str::OwnedStrInput;
pub use self::str::StrInput;

use crate::char_traits;

/// Interface for a source of characters to the YAML scanner.
///
/// The scanner operates on characters through this trait, which allows
/// input-specific optimizations (e.g., direct byte-level checks for `&str`).
pub trait Input {
    /// Ensure at least `count` characters are available for peeking.
    ///
    /// If the input is exhausted, subsequent `peek()` calls return `\0`.
    fn lookahead(&mut self, count: usize);

    /// Return the number of characters available in the buffer.
    fn buflen(&self) -> usize;

    /// Return the next character without consuming it.
    ///
    /// Returns `\0` if the input is exhausted.
    fn peek(&self) -> char;

    /// Return the `n`-th character without consuming it.
    ///
    /// The caller must have called `lookahead(n + 1)` beforehand.
    /// Returns `\0` if `n` is past the end of input.
    fn peek_nth(&self, n: usize) -> char;

    /// Consume the next character.
    fn skip(&mut self);

    /// Consume the next `count` characters.
    fn skip_n(&mut self, count: usize) {
        for _ in 0..count {
            self.skip();
        }
    }

    /// Ensure at least one character is buffered, then return it.
    #[inline]
    fn look_ch(&mut self) -> char {
        self.lookahead(1);
        self.peek()
    }

    /// Check whether the next character equals `c`.
    #[inline]
    fn next_char_is(&self, c: char) -> bool {
        self.peek() == c
    }

    /// Check whether the `n`-th character equals `c`.
    #[inline]
    fn nth_char_is(&self, n: usize, c: char) -> bool {
        self.peek_nth(n) == c
    }

    /// Check whether the next character is nil (`\0` / end of input).
    #[inline]
    fn next_is_z(&self) -> bool {
        char_traits::is_z(self.peek())
    }

    /// Check whether the next character is a line break.
    #[inline]
    fn next_is_break(&self) -> bool {
        char_traits::is_break(self.peek())
    }

    /// Check whether the next character is nil or a line break.
    #[inline]
    fn next_is_breakz(&self) -> bool {
        char_traits::is_breakz(self.peek())
    }

    /// Check whether the next character is a YAML whitespace.
    #[inline]
    fn next_is_blank(&self) -> bool {
        char_traits::is_blank(self.peek())
    }

    /// Check whether the next character is nil, a line break, or a whitespace.
    #[inline]
    fn next_is_blank_or_breakz(&self) -> bool {
        char_traits::is_blank_or_breakz(self.peek())
    }

    /// Check whether the next character is ASCII alphanumeric, `_`, or `-`.
    #[inline]
    fn next_is_alpha(&self) -> bool {
        char_traits::is_alpha(self.peek())
    }

    /// Check whether the next character is an ASCII digit.
    #[inline]
    fn next_is_digit(&self) -> bool {
        char_traits::is_digit(self.peek())
    }

    /// Check whether the next character is a BOM.
    #[inline]
    fn next_is_bom(&self) -> bool {
        char_traits::is_bom(self.peek())
    }
}
