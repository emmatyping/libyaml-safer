use crate::char_traits;
use crate::input::Input;

/// A parser input backed by an owned `String`.
///
/// This is identical to [`super::StrInput`] in performance characteristics
/// (ASCII fast-paths, no-op `lookahead()`), but owns its data so there is no
/// lifetime parameter. Use this when the caller wants to move a `String` into
/// the parser without worrying about borrow lifetimes.
pub struct OwnedStrInput {
    /// The full input string.
    data: String,
    /// Byte offset into `data` marking the start of unconsumed input.
    offset: usize,
    /// Tracks the requested lookahead count (for `buflen()`).
    lookahead: usize,
}

impl OwnedStrInput {
    /// Create a new [`OwnedStrInput`] from an owned `String`.
    #[must_use]
    pub fn new(data: String) -> Self {
        Self {
            data,
            offset: 0,
            lookahead: 0,
        }
    }

    /// Return the remaining (unconsumed) portion of the input.
    #[inline]
    fn remaining(&self) -> &str {
        &self.data[self.offset..]
    }
}

impl Input for OwnedStrInput {
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
        let buf = self.remaining();
        if buf.is_empty() {
            return '\0';
        }
        let b = buf.as_bytes()[0];
        if b < 0x80 {
            b as char
        } else {
            buf.chars().next().unwrap()
        }
    }

    #[inline]
    fn peek_nth(&self, n: usize) -> char {
        if n == 0 {
            return self.peek();
        }
        let bytes = self.remaining().as_bytes();
        // Fast path: both the first `n` characters and the target are ASCII.
        if n == 1 && bytes.len() >= 2 && bytes[0] < 0x80 && bytes[1] < 0x80 {
            return bytes[1] as char;
        }
        self.remaining().chars().nth(n).unwrap_or('\0')
    }

    #[inline]
    fn skip(&mut self) {
        let buf = self.remaining();
        if !buf.is_empty() {
            let b = buf.as_bytes()[0];
            if b < 0x80 {
                self.offset += 1;
            } else {
                let c = buf.chars().next().unwrap();
                self.offset += c.len_utf8();
            }
        }
    }

    #[inline]
    fn skip_n(&mut self, count: usize) {
        let buf = self.remaining();
        let mut chars = buf.chars();
        for _ in 0..count {
            if chars.next().is_none() {
                break;
            }
        }
        self.offset = self.data.len() - chars.as_str().len();
    }

    #[inline]
    fn next_is_z(&self) -> bool {
        self.remaining().is_empty()
    }

    #[inline]
    fn next_is_break(&self) -> bool {
        let buf = self.remaining();
        if buf.is_empty() {
            return false;
        }
        let b = buf.as_bytes()[0];
        if b < 0x80 {
            matches!(b, b'\n' | b'\r')
        } else {
            // Check for Unicode line breaks: NEL (U+0085), LS (U+2028), PS (U+2029).
            matches!(
                buf.chars().next(),
                Some('\u{0085}' | '\u{2028}' | '\u{2029}')
            )
        }
    }

    #[inline]
    fn next_is_breakz(&self) -> bool {
        self.remaining().is_empty() || self.next_is_break()
    }

    #[inline]
    fn next_is_blank(&self) -> bool {
        let buf = self.remaining();
        !buf.is_empty() && matches!(buf.as_bytes()[0], b' ' | b'\t')
    }

    #[inline]
    fn next_is_blank_or_breakz(&self) -> bool {
        let buf = self.remaining();
        if buf.is_empty() {
            return true;
        }
        let b = buf.as_bytes()[0];
        if b < 0x80 {
            matches!(b, b' ' | b'\t' | b'\n' | b'\r')
        } else {
            char_traits::is_break(buf.chars().next().unwrap())
        }
    }

    #[inline]
    fn next_is_alpha(&self) -> bool {
        let buf = self.remaining();
        !buf.is_empty()
            && matches!(
                buf.as_bytes()[0],
                b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'-'
            )
    }

    #[inline]
    fn next_is_digit(&self) -> bool {
        let buf = self.remaining();
        !buf.is_empty() && buf.as_bytes()[0].is_ascii_digit()
    }

    #[inline]
    fn next_is_bom(&self) -> bool {
        // BOM is U+FEFF, which is 0xEF 0xBB 0xBF in UTF-8.
        self.remaining().starts_with('\u{FEFF}')
    }
}

impl From<String> for OwnedStrInput {
    fn from(value: String) -> Self {
        OwnedStrInput::new(value)
    }
}
