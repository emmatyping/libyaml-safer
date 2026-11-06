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
    /// passes `after_blanks` and is at most `max_word_start` characters from
    /// the front. It never ends in blanks.
    #[inline]
    pub(crate) fn run_words(
        &self,
        max: usize,
        max_word_start: usize,
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
            } else if ordinary(ch) && (!blanks || (end.chars <= max_word_start && after_blanks(ch)))
            {
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

    /// The longest leading run of at most `max` characters that are printable
    /// ASCII or blanks and not in `stops`, ending before any trailing blanks.
    #[inline]
    pub(crate) fn plain_ascii_words(&self, max: usize, stops: &[u8]) -> Run {
        let bytes = &self.text.as_bytes()[self.pos..];
        let bytes = &bytes[..max.min(bytes.len())];
        let span = &bytes[..crate::simd::plain_ascii_prefix(bytes, stops)];
        let end = span
            .iter()
            .rposition(|&b| b != b' ' && b != b'\t')
            .map_or(0, |last| last + 1);
        Run {
            chars: end,
            bytes: end,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str) -> CharBuffer {
        let mut buffer = CharBuffer::with_capacity(0);
        buffer.push_str(text, text.chars().count());
        buffer
    }

    #[test]
    fn get_counts_characters() {
        let buffer = buffer("aé中𝄞b");
        assert_eq!(buffer.len(), 5);
        let chars: Vec<_> = (0..6).map(|i| buffer.get(i)).collect();
        assert_eq!(
            chars,
            [Some('a'), Some('é'), Some('中'), Some('𝄞'), Some('b'), None]
        );
    }

    #[test]
    fn pop_skip_and_take() {
        let mut buffer = buffer("é中ab𝄞c");
        assert_eq!(buffer.pop_front(), Some('é'));
        buffer.skip(2);
        assert_eq!(buffer.front(), Some('b'));
        let run = buffer.run(usize::MAX, |ch| ch != 'c');
        assert_eq!(run.chars, 2);
        let mut taken = String::new();
        buffer.take_run(run, &mut taken);
        assert_eq!(taken, "b𝄞");
        assert_eq!((buffer.len(), buffer.front()), (1, Some('c')));
    }

    #[test]
    fn run_stops_at_max() {
        let buffer = buffer("中中中中");
        assert_eq!(buffer.run(2, |_| true).chars, 2);
    }

    #[test]
    fn run_words_never_ends_in_blanks() {
        let words = |text: &str, max: usize| {
            let mut buffer = buffer(text);
            let run = buffer.run_words(max, usize::MAX, |ch| ch.is_alphanumeric(), |ch| ch != '#');
            let mut taken = String::new();
            buffer.take_run(run, &mut taken);
            taken
        };
        assert_eq!(words("ab cd\t é  ;", usize::MAX), "ab cd\t é");
        assert_eq!(words("ab  #cd", usize::MAX), "ab");
        assert_eq!(words("ab cd", 4), "ab c");
        assert_eq!(words("ab cd", 3), "ab");
        assert_eq!(words("ab   ", usize::MAX), "ab");
    }

    #[test]
    fn run_words_limits_where_words_start() {
        let buffer = buffer("ab cd ef");
        let run = buffer.run_words(usize::MAX, 3, |ch| ch.is_alphabetic(), |_| true);
        assert_eq!(run.chars, 5);
        let run = buffer.run_words(usize::MAX, 2, |ch| ch.is_alphabetic(), |_| true);
        assert_eq!(run.chars, 2);
    }

    #[test]
    fn compacts_consumed_text() {
        let mut buffer = buffer("abcd");
        buffer.skip(3);
        buffer.push_str("é", 1);
        assert_eq!(buffer.pos, 0);
        assert_eq!((buffer.get(0), buffer.get(1)), (Some('d'), Some('é')));
    }
}
