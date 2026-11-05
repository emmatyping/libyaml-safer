//! The scanner's working buffer of decoded characters.

/// Characters at the front of a [`CharBuffer`].
#[derive(Clone, Copy)]
pub(crate) struct Run {
    pub(crate) chars: usize,
    bytes: usize,
}

/// Decoded characters waiting to be scanned, stored as UTF-8.
///
/// This replaces a queue of `char`s: the scanner mostly looks a few
/// characters ahead and consumes runs of them into token values, which is a
/// copy of a string slice here.
pub(crate) struct CharBuffer {
    text: String,
    /// Byte offset of the first unconsumed character.
    pos: usize,
    /// Number of unconsumed characters.
    len: usize,
}

impl CharBuffer {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            text: String::with_capacity(capacity),
            pos: 0,
            len: 0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.pos = 0;
        self.len = 0;
    }

    /// The number of characters in the buffer.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    fn rest(&self) -> &str {
        &self.text[self.pos..]
    }

    /// The character `index` characters from the front.
    #[inline(always)]
    pub(crate) fn get(&self, index: usize) -> Option<char> {
        let bytes = self.text.as_bytes();
        let mut at = self.pos;
        for _ in 0..index {
            at += utf8_width(*bytes.get(at)?);
        }
        let &lead = bytes.get(at)?;
        if lead < 0x80 {
            Some(char::from(lead))
        } else {
            self.decode(at)
        }
    }

    #[inline(never)]
    fn decode(&self, at: usize) -> Option<char> {
        self.text[at..].chars().next()
    }

    #[inline(always)]
    pub(crate) fn front(&self) -> Option<char> {
        self.get(0)
    }

    #[inline]
    pub(crate) fn pop_front(&mut self) -> Option<char> {
        let ch = self.front()?;
        self.pos += ch.len_utf8();
        self.len -= 1;
        Some(ch)
    }

    /// Drop the first `count` characters (which must be in the buffer).
    #[inline]
    pub(crate) fn skip(&mut self, count: usize) {
        let bytes = self.byte_len(count);
        self.pos += bytes;
        self.len -= count;
    }

    /// The byte length of the first `count` characters.
    #[inline]
    fn byte_len(&self, count: usize) -> usize {
        let rest = self.rest();
        if rest.as_bytes()[..count.min(rest.len())].is_ascii() {
            return count;
        }
        rest.char_indices()
            .nth(count)
            .map_or(rest.len(), |(offset, _)| offset)
    }

    /// The leading run of at most `max` characters that are `ordinary`.
    #[inline]
    pub(crate) fn run(&self, max: usize, ordinary: impl Fn(char) -> bool) -> Run {
        let bytes = &self.text.as_bytes()[self.pos..];
        let mut run = Run { chars: 0, bytes: 0 };
        while run.chars < max {
            let Some(&lead) = bytes.get(run.bytes) else {
                break;
            };
            let ch = if lead < 0x80 {
                char::from(lead)
            } else {
                match self.decode(self.pos + run.bytes) {
                    Some(ch) => ch,
                    None => break,
                }
            };
            if !ordinary(ch) {
                break;
            }
            run.chars += 1;
            run.bytes += ch.len_utf8();
        }
        run
    }

    /// Like `run`, but the run may also contain spaces and tabs between
    /// `ordinary` characters, where the character after the blanks also
    /// passes `after_blanks`. It never ends in blanks.
    #[inline]
    pub(crate) fn run_words(
        &self,
        max: usize,
        ordinary: impl Fn(char) -> bool,
        after_blanks: impl Fn(char) -> bool,
    ) -> Run {
        let mut run = Run { chars: 0, bytes: 0 };
        // The run so far plus blanks that are not part of it yet.
        let mut end = run;
        let mut blanks = false;
        while end.chars < max {
            let Some(ch) = self.char_at_byte(end.bytes) else {
                break;
            };
            if ch == ' ' || ch == '\t' {
                blanks = true;
            } else if ordinary(ch) && (!blanks || after_blanks(ch)) {
                blanks = false;
            } else {
                break;
            }
            end.chars += 1;
            end.bytes += ch.len_utf8();
            if !blanks {
                run = end;
            }
        }
        run
    }

    /// The character at byte offset `at` of the unconsumed text.
    #[inline(always)]
    fn char_at_byte(&self, at: usize) -> Option<char> {
        let &lead = self.text.as_bytes().get(self.pos + at)?;
        if lead < 0x80 {
            Some(char::from(lead))
        } else {
            self.decode(self.pos + at)
        }
    }

    /// Drop a run from the front.
    #[inline]
    pub(crate) fn skip_run(&mut self, run: Run) {
        self.pos += run.bytes;
        self.len -= run.chars;
    }

    /// Move a run from the front to the end of `string`.
    #[inline]
    pub(crate) fn take_run(&mut self, run: Run, string: &mut String) {
        string.push_str(&self.text[self.pos..self.pos + run.bytes]);
        self.skip_run(run);
    }

    /// Make room before appending, dropping consumed text once it is at least
    /// half the buffer so the buffer does not grow with the input.
    fn compact(&mut self) {
        if self.pos > 0 && self.pos >= self.text.len() / 2 {
            self.text.drain(..self.pos);
            self.pos = 0;
        }
    }

    pub(crate) fn push_back(&mut self, ch: char) {
        self.compact();
        self.text.push(ch);
        self.len += 1;
    }

    /// Append `text`, which holds `chars` characters.
    pub(crate) fn push_str(&mut self, text: &str, chars: usize) {
        self.compact();
        self.text.push_str(text);
        self.len += chars;
    }
}

/// The width of the UTF-8 sequence starting with `lead`.
#[inline]
fn utf8_width(lead: u8) -> usize {
    match lead {
        0x00..=0x7f => 1,
        0x80..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}
