//! Byte scans done 16 bytes at a time with SIMD.

use fearless_simd::{dispatch, prelude::*, u8x16, Level};

/// Whether `byte` is printable ASCII, a tab or a line break: the ASCII the
/// reader accepts.
#[inline(always)]
fn is_allowed_ascii(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7e | b'\t' | b'\n' | b'\r')
}

/// The length of the leading run of `bytes` that `is_allowed_ascii`.
pub(crate) fn allowed_ascii_prefix(bytes: &[u8]) -> usize {
    if bytes.len() < 16 {
        return scalar_prefix(bytes, 0);
    }
    dispatch!(Level::new(), simd => allowed_ascii_prefix_simd(simd, bytes))
}

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
    scalar_prefix(bytes, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_ascii_prefix_matches_scalar() {
        for len in 0..80 {
            for bad_at in 0..=len {
                for bad in [0u8, 8, 0x0b, 0x1f, 0x7f, 0x80, 0xff] {
                    let mut bytes = vec![b'a'; len];
                    for (i, b) in bytes.iter_mut().enumerate() {
                        *b = [b'a', b' ', b'\t', b'\n', b'\r', b'~'][i % 6];
                    }
                    if bad_at < len {
                        bytes[bad_at] = bad;
                    }
                    assert_eq!(allowed_ascii_prefix(&bytes), scalar_prefix(&bytes, 0));
                }
            }
        }
    }
}
