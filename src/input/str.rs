use crate::char_traits;
use crate::input::Input;

/// A parser input backed by a `&str`.
///
/// This is the most efficient input source: the entire string is available,
/// so `lookahead()` is a no-op, and `peek()`/`skip()` use ASCII fast-paths
/// that operate directly on bytes when possible.
pub struct StrInput<'a> {
    /// The remaining input (a moving window into the original string).
    buffer: &'a str,
    /// Tracks the requested lookahead count (for `buflen()`).
    lookahead: usize,
}

impl<'a> StrInput<'a> {
    /// Create a new [`StrInput`] from a string slice.
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            buffer: input,
            lookahead: 0,
        }
    }
}

impl Input for StrInput<'_> {
    #[inline]
    fn lookahead(&mut self, count: usize) {
        // The entire string is already available; just track the request.
        self.lookahead = self.lookahead.max(count);
    }

    #[inline]
    fn buflen(&self) -> usize {
        self.lookahead
    }

    #[inline]
    fn peek(&self) -> char {
        if self.buffer.is_empty() {
            return '\0';
        }
        let b = self.buffer.as_bytes()[0];
        if b < 0x80 {
            b as char
        } else {
            self.buffer.chars().next().unwrap()
        }
    }

    #[inline]
    fn peek_nth(&self, n: usize) -> char {
        if n == 0 {
            return self.peek();
        }
        let bytes = self.buffer.as_bytes();
        // Fast path: both the first `n` characters and the target are ASCII.
        if n == 1 && bytes.len() >= 2 && bytes[0] < 0x80 && bytes[1] < 0x80 {
            return bytes[1] as char;
        }
        self.buffer.chars().nth(n).unwrap_or('\0')
    }

    #[inline]
    fn skip(&mut self) {
        if !self.buffer.is_empty() {
            let b = self.buffer.as_bytes()[0];
            if b < 0x80 {
                self.buffer = &self.buffer[1..];
            } else {
                let mut chars = self.buffer.chars();
                chars.next();
                self.buffer = chars.as_str();
            }
        }
    }

    #[inline]
    fn skip_n(&mut self, count: usize) {
        let mut chars = self.buffer.chars();
        for _ in 0..count {
            if chars.next().is_none() {
                break;
            }
        }
        self.buffer = chars.as_str();
    }

    #[inline]
    fn next_is_z(&self) -> bool {
        self.buffer.is_empty()
    }

    #[inline]
    fn next_is_break(&self) -> bool {
        if self.buffer.is_empty() {
            return false;
        }
        let b = self.buffer.as_bytes()[0];
        if b < 0x80 {
            matches!(b, b'\n' | b'\r')
        } else {
            // Check for Unicode line breaks: NEL (U+0085), LS (U+2028), PS (U+2029).
            matches!(
                self.buffer.chars().next(),
                Some('\u{0085}' | '\u{2028}' | '\u{2029}')
            )
        }
    }

    #[inline]
    fn next_is_breakz(&self) -> bool {
        self.buffer.is_empty() || self.next_is_break()
    }

    #[inline]
    fn next_is_blank(&self) -> bool {
        !self.buffer.is_empty() && matches!(self.buffer.as_bytes()[0], b' ' | b'\t')
    }

    #[inline]
    fn next_is_blank_or_breakz(&self) -> bool {
        if self.buffer.is_empty() {
            return true;
        }
        let b = self.buffer.as_bytes()[0];
        if b < 0x80 {
            matches!(b, b' ' | b'\t' | b'\n' | b'\r')
        } else {
            char_traits::is_break(self.buffer.chars().next().unwrap())
        }
    }

    #[inline]
    fn next_is_alpha(&self) -> bool {
        !self.buffer.is_empty()
            && matches!(
                self.buffer.as_bytes()[0],
                b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'-'
            )
    }

    #[inline]
    fn next_is_digit(&self) -> bool {
        !self.buffer.is_empty() && self.buffer.as_bytes()[0].is_ascii_digit()
    }

    #[inline]
    fn next_is_bom(&self) -> bool {
        // BOM is U+FEFF, which is 0xEF 0xBB 0xBF in UTF-8.
        self.buffer.starts_with('\u{FEFF}')
    }
}

impl<'a> From<&'a str> for StrInput<'a> {
    fn from(value: &'a str) -> Self {
        StrInput::new(value)
    }
}
