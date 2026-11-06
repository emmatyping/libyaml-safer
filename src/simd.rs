//! Byte scans done 16 bytes at a time with SIMD. 16-byte vectors are in
//! every x86-64 and aarch64 baseline, so these use the baseline level, which
//! needs no runtime dispatch and can be inlined into the scanner.

use fearless_simd::{dispatch, prelude::*, u8x16, Level};

/// Whether `byte` is printable ASCII, a tab or a line break: the ASCII the
/// reader accepts.
#[cfg(test)]
#[inline(always)]
fn is_allowed_ascii(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7e | b'\t' | b'\n' | b'\r')
}

/// The length of the leading run of `bytes` that are printable ASCII, tabs
/// or line breaks: the ASCII the reader accepts.
pub(crate) fn allowed_ascii_prefix(bytes: &[u8]) -> usize {
    dispatch!(Level::baseline(), simd => allowed_ascii_prefix_simd(simd, bytes))
}

#[cfg(test)]
#[inline(always)]
fn scalar_prefix(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|&b| !is_allowed_ascii(b))
        .map_or(bytes.len(), |offset| start + offset)
}

#[inline(always)]
fn allowed_ascii_prefix_simd<S: Simd>(simd: S, bytes: &[u8]) -> usize {
    let mut chunks = bytes.chunks_exact(16);
    let mut start = 0;
    for chunk in &mut chunks {
        let v = u8x16::from_slice(simd, chunk);
        let allowed = (v.simd_ge(0x20) & v.simd_lt(0x7f))
            | v.simd_eq(b'\t')
            | v.simd_eq(b'\n')
            | v.simd_eq(b'\r');
        let bits = allowed.to_bitmask();
        if bits != 0xffff {
            return start + bits.trailing_ones() as usize;
        }
        start += 16;
    }
    // The rest, padded with NULs, which are not allowed.
    let v = u8x16::from_slice(simd, &padded(chunks.remainder()));
    let allowed = (v.simd_ge(0x20) & v.simd_lt(0x7f))
        | v.simd_eq(b'\t')
        | v.simd_eq(b'\n')
        | v.simd_eq(b'\r');
    start + (allowed.to_bitmask().trailing_ones() as usize).min(chunks.remainder().len())
}

/// `tail` (shorter than 16 bytes) followed by NULs.
#[inline(always)]
fn padded(tail: &[u8]) -> [u8; 16] {
    let mut block = [0; 16];
    block[..tail.len()].copy_from_slice(tail);
    block
}

/// Printable ASCII and tabs, other than some `stops`.
pub(crate) struct PlainAscii {
    stops: &'static [u8],
    /// Whether each byte is in the class.
    table: [bool; 256],
}

impl PlainAscii {
    pub(crate) const fn new(stops: &'static [u8]) -> Self {
        let mut table = [false; 256];
        let mut byte = 0;
        while byte < 256 {
            table[byte] = byte == b'\t' as usize || (byte >= 0x20 && byte < 0x7f);
            byte += 1;
        }
        let mut i = 0;
        while i < stops.len() {
            table[stops[i] as usize] = false;
            i += 1;
        }
        Self { stops, table }
    }

    /// Whether `byte` is in the class.
    #[inline(always)]
    pub(crate) fn contains(&self, byte: u8) -> bool {
        self.table[byte as usize]
    }

    /// The length of the leading run of `bytes` in the class. Short runs are
    /// common, so the first few bytes are checked one at a time.
    #[inline(always)]
    pub(crate) fn prefix(&self, bytes: &[u8]) -> usize {
        const SCALAR: usize = 8;
        let head = &bytes[..bytes.len().min(SCALAR)];
        match head.iter().position(|&b| !self.contains(b)) {
            Some(len) => len,
            None if head.len() < SCALAR => head.len(),
            None => SCALAR + plain_ascii_prefix(&bytes[SCALAR..], self.stops),
        }
    }
}

/// The length of the leading run of `bytes` that are printable ASCII or tabs
/// and not in `stops`.
#[inline(always)]
fn plain_ascii_prefix(bytes: &[u8], stops: &[u8]) -> usize {
    dispatch!(Level::baseline(), simd => plain_ascii_prefix_simd(simd, bytes, stops))
}

#[cfg(test)]
#[inline(always)]
fn is_plain_ascii(byte: u8, stops: &[u8]) -> bool {
    matches!(byte, 0x20..=0x7e | b'\t') && !stops.contains(&byte)
}

#[cfg(test)]
#[inline(always)]
fn scalar_plain_prefix(bytes: &[u8], start: usize, stops: &[u8]) -> usize {
    bytes[start..]
        .iter()
        .position(|&b| !is_plain_ascii(b, stops))
        .map_or(bytes.len(), |offset| start + offset)
}

#[inline(always)]
fn plain_ascii_prefix_simd<S: Simd>(simd: S, bytes: &[u8], stops: &[u8]) -> usize {
    #[inline(always)]
    fn stop_bits<S: Simd>(v: u8x16<S>, stops: &[u8]) -> u64 {
        let mut stop = v.simd_lt(0x20) & !v.simd_eq(b'\t') | v.simd_ge(0x7f);
        for &byte in stops {
            stop = stop | v.simd_eq(byte);
        }
        stop.to_bitmask()
    }
    let mut chunks = bytes.chunks_exact(16);
    let mut start = 0;
    for chunk in &mut chunks {
        let bits = stop_bits(u8x16::from_slice(simd, chunk), stops);
        if bits != 0 {
            return start + bits.trailing_zeros() as usize;
        }
        start += 16;
    }
    // The rest, padded with NULs, which stop the run.
    let tail = chunks.remainder();
    let bits = stop_bits(u8x16::from_slice(simd, &padded(tail)), stops);
    start + (bits.trailing_zeros() as usize).min(tail.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_ascii_prefix_matches_scalar() {
        for len in 0..80 {
            for bad_at in 0..=len {
                for bad in [
                    0u8, 8, 0x0b, 0x1f, 0x7f, 0x80, 0xff, b':', b'"', b'\\', b'{',
                ] {
                    let mut bytes = vec![b'a'; len];
                    for (i, b) in bytes.iter_mut().enumerate() {
                        *b = [b'a', b' ', b'\t', b'\n', b'\r', b'~'][i % 6];
                    }
                    if bad_at < len {
                        bytes[bad_at] = bad;
                    }
                    assert_eq!(allowed_ascii_prefix(&bytes), scalar_prefix(&bytes, 0));
                    for class in [
                        &PlainAscii::new(b""),
                        &PlainAscii::new(b"'\"\\"),
                        &PlainAscii::new(b":#,[]{}"),
                    ] {
                        assert_eq!(
                            class.prefix(&bytes),
                            scalar_plain_prefix(&bytes, 0, class.stops)
                        );
                    }
                }
            }
        }
    }
}
