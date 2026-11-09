//! Byte scans done many bytes at a time with SIMD.
//!
//! Runs are scanned 16, 32 or 64 bytes at a time, as chosen at runtime from
//! the CPU's features and the length of the input: 64 with AVX-512, 32 with
//! AVX2 and 16 otherwise, but never wider than the input. Targets with no SIMD
//! scan one byte at a time.
//!
//! Most runs are short, so the first 16 bytes of a run are checked inline
//! with the baseline level (16-byte vectors are in every x86-64 and aarch64
//! baseline), which needs no runtime dispatch. Only runs longer than that use
//! the runtime-selected width.
//!
//! The inline check compares bytes with each byte the class stops at, which
//! the compiler can specialize for each class. The runtime-selected scans
//! instead look up both nibbles of each byte in 16-byte tables, as simdjson
//! does, which costs the same for every class. Levels that cannot look up
//! bytes in a vector quickly (SSE2 without SSSE3) compare bytes instead.

use std::sync::OnceLock;

use fearless_simd::{dispatch, prelude::*, u8x16, u8x32, u8x64, Level};

/// A set of ASCII bytes that runs are made of.
pub(crate) trait ByteClass {
    /// Whether `byte` is in the class.
    fn contains(&self, byte: u8) -> bool;

    /// A bitmask of the lanes of `v` that are not in the class, found by
    /// comparing them with the bytes the class stops at.
    fn stop_bits<S: Simd, V: SimdInt<S, Element = u8>>(&self, simd: S, v: V) -> u64;

    /// The class as a table for `Lookup`.
    fn nibbles(&self) -> &[u8; 16];
}

/// Whether each byte is printable ASCII, a tab or a line break: the ASCII the
/// reader accepts.
const fn allowed_ascii_table() -> [bool; 256] {
    let mut table = [false; 256];
    let mut byte = 0;
    while byte < 256 {
        table[byte] = matches!(byte as u8, 0x20..=0x7e | b'\t' | b'\n' | b'\r');
        byte += 1;
    }
    table
}

/// Printable ASCII, tabs and line breaks: the ASCII the reader accepts.
struct AllowedAscii {
    nibbles: [u8; 16],
}

static ALLOWED_ASCII: AllowedAscii = AllowedAscii {
    nibbles: nibble_table(&allowed_ascii_table()),
};

impl ByteClass for AllowedAscii {
    #[inline(always)]
    fn contains(&self, byte: u8) -> bool {
        matches!(byte, 0x20..=0x7e | b'\t' | b'\n' | b'\r')
    }

    #[inline(always)]
    fn stop_bits<S: Simd, V: SimdInt<S, Element = u8>>(&self, simd: S, v: V) -> u64 {
        let allowed = (v.simd_ge(V::splat(simd, 0x20)) & v.simd_lt(V::splat(simd, 0x7f)))
            | v.simd_eq(V::splat(simd, b'\t'))
            | v.simd_eq(V::splat(simd, b'\n'))
            | v.simd_eq(V::splat(simd, b'\r'));
        (!allowed).to_bitmask()
    }

    fn nibbles(&self) -> &[u8; 16] {
        &self.nibbles
    }
}

/// The length of the leading run of `bytes` that are printable ASCII, tabs
/// or line breaks: the ASCII the reader accepts.
#[inline(always)]
pub(crate) fn allowed_ascii_prefix(bytes: &[u8]) -> usize {
    prefix(bytes, &ALLOWED_ASCII)
}

/// Printable ASCII and tabs, other than some `stops`.
pub(crate) struct PlainAscii {
    stops: &'static [u8],
    /// Whether each byte is in the class.
    table: [bool; 256],
    nibbles: [u8; 16],
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
        let nibbles = nibble_table(&table);
        Self {
            stops,
            table,
            nibbles,
        }
    }

    /// The length of the leading run of `bytes` in the class.
    #[inline(always)]
    pub(crate) fn prefix(&self, bytes: &[u8]) -> usize {
        prefix(bytes, self)
    }

    /// `prefix`, for runs that are usually short: the first few bytes are
    /// checked one at a time.
    #[inline(always)]
    pub(crate) fn short_prefix(&self, bytes: &[u8]) -> usize {
        const SCALAR: usize = 8;
        let head = &bytes[..bytes.len().min(SCALAR)];
        match head.iter().position(|&b| !self.contains(b)) {
            Some(len) => len,
            None if head.len() < SCALAR => head.len(),
            None => SCALAR + self.prefix(&bytes[SCALAR..]),
        }
    }
}

impl ByteClass for PlainAscii {
    #[inline(always)]
    fn contains(&self, byte: u8) -> bool {
        self.table[byte as usize]
    }

    #[inline(always)]
    fn stop_bits<S: Simd, V: SimdInt<S, Element = u8>>(&self, simd: S, v: V) -> u64 {
        let mut stop = v.simd_lt(V::splat(simd, 0x20)) & !v.simd_eq(V::splat(simd, b'\t'))
            | v.simd_ge(V::splat(simd, 0x7f));
        for &byte in self.stops {
            stop |= v.simd_eq(V::splat(simd, byte));
        }
        stop.to_bitmask()
    }

    fn nibbles(&self) -> &[u8; 16] {
        &self.nibbles
    }
}

/// The `ByteClass::nibbles` table of the ASCII bytes in `table`: for each low
/// nibble, a bit for each high nibble (0 to 7) that makes a byte in the class.
const fn nibble_table(table: &[bool; 256]) -> [u8; 16] {
    let mut nibbles = [0; 16];
    let mut byte = 0;
    while byte < 0x80 {
        if table[byte] {
            nibbles[byte & 0xf] |= 1 << (byte >> 4);
        }
        byte += 1;
    }
    nibbles
}

/// The bit of each high nibble in a `ByteClass::nibbles` table. Bytes with
/// the high bit set are in no class.
const HIGH_NIBBLES: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 0, 0, 0, 0, 0, 0, 0, 0];

/// A vector of bytes, of any width.
trait Bytes<S: Simd>: SimdInt<S, Element = u8, Block = u8x16<S>, ByteVector = Self> {}

impl<S: Simd, V: SimdInt<S, Element = u8, Block = u8x16<S>, ByteVector = V>> Bytes<S> for V {}

/// A way of finding the lanes of a vector that end a run.
trait Stops<S: Simd> {
    /// A bitmask of the lanes of `v` that are not in the class.
    fn bits<V: Bytes<S>>(&self, v: V) -> u64;
}

/// Finds the end of a run with `ByteClass::stop_bits`.
struct Compare<'a, S, C> {
    simd: S,
    class: &'a C,
}

impl<S: Simd, C: ByteClass> Stops<S> for Compare<'_, S, C> {
    #[inline(always)]
    fn bits<V: Bytes<S>>(&self, v: V) -> u64 {
        self.class.stop_bits(self.simd, v)
    }
}

/// Finds the end of a run by looking up each byte's nibbles in a class's
/// `ByteClass::nibbles` table and `HIGH_NIBBLES`: the byte is in the class
/// if the two entries share a bit.
struct Lookup<S: Simd> {
    simd: S,
    low: u8x16<S>,
    high: u8x16<S>,
}

impl<S: Simd> Lookup<S> {
    #[inline(always)]
    fn new<C: ByteClass>(simd: S, class: &C) -> Self {
        Lookup {
            simd,
            low: u8x16::from_slice(simd, class.nibbles()),
            high: u8x16::from_slice(simd, &HIGH_NIBBLES),
        }
    }
}

impl<S: Simd> Stops<S> for Lookup<S> {
    #[inline(always)]
    fn bits<V: Bytes<S>>(&self, v: V) -> u64 {
        let low = V::block_splat(self.low).swizzle_dyn_within_blocks(v & 0x0f);
        let high = V::block_splat(self.high).swizzle_dyn_within_blocks(v >> 4);
        (low & high).simd_eq(V::splat(self.simd, 0)).to_bitmask()
    }
}

/// Whether `level` can look up bytes in a vector in one instruction.
#[inline(always)]
fn has_byte_lookup(level: Level) -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    return level.as_sse4_2().is_some();
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    return !level.is_fallback();
}

/// The length of the leading run of `bytes` in `class`: the first 16 bytes
/// are checked inline at the baseline level, and the rest with the
/// runtime-selected width.
#[inline(always)]
fn prefix<C: ByteClass>(bytes: &[u8], class: &C) -> usize {
    if Level::baseline().is_fallback() {
        return scalar_prefix(bytes, class);
    }
    let head = &bytes[..bytes.len().min(16)];
    let len =
        dispatch!(Level::baseline(), simd => block_prefix(simd, head, &Compare { simd, class }));
    if len < 16 || bytes.len() == 16 {
        len
    } else {
        selected_prefix(bytes, 16, class)
    }
}

/// How many bytes are scanned at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Width {
    /// One at a time, without SIMD.
    Scalar,
    W16,
    W32,
    W64,
}

/// The level to scan with at each width, on this CPU.
#[derive(Clone, Copy, Debug)]
struct Levels {
    /// AVX-512, for 64 bytes at a time.
    w64: Option<Level>,
    /// AVX2, for 32 bytes at a time.
    w32: Option<Level>,
    /// The best level below AVX-512, for 16 bytes at a time; `None` if there
    /// is no SIMD.
    w16: Option<Level>,
}

impl Levels {
    /// The levels to use when `best` is the best level the CPU has.
    fn new(best: Level) -> Self {
        if best.is_fallback() {
            return Levels {
                w64: None,
                w32: None,
                w16: None,
            };
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let w32 = best.as_avx2().map(Simd::level);
            Levels {
                w64: best.as_avx512().map(Simd::level),
                w32,
                w16: Some(w32.unwrap_or(best)),
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        Levels {
            w64: None,
            w32: None,
            w16: Some(best),
        }
    }

    /// Detected once, on first use.
    fn get() -> Self {
        static LEVELS: OnceLock<Levels> = OnceLock::new();
        *LEVELS.get_or_init(|| Levels::new(Level::new()))
    }

    /// The width and level to scan a run of up to `len` bytes with: the
    /// widest vectors that fit in it. Wider vectors cost more to set up, so
    /// short runs use narrow ones even when wide ones are available.
    fn choose(self, len: usize) -> (Level, Width) {
        match (self.w64, self.w32, self.w16) {
            (Some(level), _, _) if len >= 64 => (level, Width::W64),
            (_, Some(level), _) if len >= 32 => (level, Width::W32),
            (_, _, Some(level)) => (level, Width::W16),
            (_, _, None) => (Level::new(), Width::Scalar),
        }
    }
}

/// The end of the run in `class` that starts at `start` in `bytes`, scanned
/// with the width and level chosen for this CPU and length. `bytes` has at
/// least 16 bytes.
#[inline(never)]
fn selected_prefix<C: ByteClass>(bytes: &[u8], start: usize, class: &C) -> usize {
    let (level, width) = Levels::get().choose(bytes.len() - start);
    prefix_with(level, width, bytes, start, class)
}

/// The end of the run in `class` that starts at `start` in `bytes`, scanned
/// at `level` and `width`. If `start` is not 0, `bytes` has at least 16
/// bytes.
#[inline(always)]
fn prefix_with<C: ByteClass>(
    level: Level,
    width: Width,
    bytes: &[u8],
    start: usize,
    class: &C,
) -> usize {
    match width {
        Width::Scalar => start + scalar_prefix(&bytes[start..], class),
        Width::W16 => dispatch!(level, simd => scan::<_, u8x16<_>, _>(simd, bytes, start, class)),
        Width::W32 => dispatch!(level, simd => scan::<_, u8x32<_>, _>(simd, bytes, start, class)),
        Width::W64 => dispatch!(level, simd => scan::<_, u8x64<_>, _>(simd, bytes, start, class)),
    }
}

#[inline(always)]
fn scalar_prefix<C: ByteClass>(bytes: &[u8], class: &C) -> usize {
    bytes
        .iter()
        .position(|&b| !class.contains(b))
        .unwrap_or(bytes.len())
}

/// The end of the run in `class` that starts at `start` in `bytes`, scanned
/// `V::LEN` bytes at a time, then 16 at a time.
#[inline(always)]
fn scan<S: Simd, V: Bytes<S>, C: ByteClass>(
    simd: S,
    bytes: &[u8],
    start: usize,
    class: &C,
) -> usize {
    if has_byte_lookup(simd.level()) {
        simd_prefix::<S, V, _>(simd, bytes, start, &Lookup::new(simd, class))
    } else {
        simd_prefix::<S, V, _>(simd, bytes, start, &Compare { simd, class })
    }
}

#[inline(always)]
fn simd_prefix<S: Simd, V: Bytes<S>, T: Stops<S>>(
    simd: S,
    bytes: &[u8],
    mut start: usize,
    stops: &T,
) -> usize {
    let len = bytes.len();
    if len < 16 {
        return padded_prefix(simd, bytes, stops);
    }
    while len - start >= V::LEN {
        let bits = stops.bits(V::from_slice(simd, &bytes[start..start + V::LEN]));
        if bits != 0 {
            return start + bits.trailing_zeros() as usize;
        }
        start += V::LEN;
    }
    while len - start >= 16 {
        let bits = stops.bits(u8x16::from_slice(simd, &bytes[start..start + 16]));
        if bits != 0 {
            return start + bits.trailing_zeros() as usize;
        }
        start += 16;
    }
    if start == len {
        return len;
    }
    // The last 16 bytes, some of which were scanned already. Loading them
    // in place is quicker than copying the rest into a padded block.
    let last = len - 16;
    let bits = stops.bits(u8x16::from_slice(simd, &bytes[last..])) >> (start - last);
    start + (bits.trailing_zeros() as usize).min(len - start)
}

/// The length of the leading run of `block` (at most 16 bytes) in the class
/// `stops` finds the end of.
#[inline(always)]
fn block_prefix<S: Simd, T: Stops<S>>(simd: S, block: &[u8], stops: &T) -> usize {
    match <&[u8; 16]>::try_from(block) {
        Ok(full) => (stops.bits(u8x16::from_slice(simd, full)).trailing_zeros() as usize).min(16),
        Err(_) => padded_prefix(simd, block, stops),
    }
}

/// The length of the leading run of `bytes` (fewer than 16) in the class
/// `stops` finds the end of.
#[inline(always)]
fn padded_prefix<S: Simd, T: Stops<S>>(simd: S, bytes: &[u8], stops: &T) -> usize {
    // Padded with NULs, which are in no class.
    let mut padded = [0; 16];
    padded[..bytes.len()].copy_from_slice(bytes);
    let bits = stops.bits(u8x16::from_slice(simd, &padded));
    (bits.trailing_zeros() as usize).min(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every level this CPU has, from the fallback up.
    fn levels() -> Vec<Level> {
        let best = Level::new();
        let mut levels = vec![Level::fallback()];
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            levels.extend(best.as_sse2().map(Simd::level));
            levels.extend(best.as_sse4_2().map(Simd::level));
            levels.extend(best.as_avx2().map(Simd::level));
            levels.extend(best.as_avx512().map(Simd::level));
        }
        #[cfg(target_arch = "aarch64")]
        levels.extend(best.as_neon().map(Simd::level));
        levels
    }

    fn check<C: ByteClass>(bytes: &[u8], class: &C) {
        let starts = [0, 1, 15, 16, 17, 40];
        for start in starts
            .into_iter()
            .filter(|&start| start == 0 || (start <= bytes.len() && bytes.len() >= 16))
        {
            let expected = start + scalar_prefix(&bytes[start..], class);
            for level in levels() {
                for width in [Width::Scalar, Width::W16, Width::W32, Width::W64] {
                    assert_eq!(
                        prefix_with(level, width, bytes, start, class),
                        expected,
                        "{level:?} {width:?} {start} {bytes:?}"
                    );
                }
            }
        }
    }

    fn classes() -> [PlainAscii; 4] {
        [
            PlainAscii::new(b""),
            PlainAscii::new(b"'\"\\"),
            PlainAscii::new(b":#,[]{}"),
            PlainAscii::new(b"\t,?[]{}:#"),
        ]
    }

    #[test]
    fn prefixes_match_scalar() {
        let classes = classes();
        for len in 0..200 {
            for bad_at in (0..=len).filter(|&i| i < 20 || i % 7 == 0 || i + 3 > len) {
                for bad in [
                    0u8, 8, 0x0b, 0x1f, 0x7f, 0x80, 0xff, b':', b'"', b'\\', b'{',
                ] {
                    let mut bytes: Vec<u8> = (0..len).map(|i| b"a \t\n\r~"[i % 6]).collect();
                    if bad_at < len {
                        bytes[bad_at] = bad;
                    }
                    check(&bytes, &ALLOWED_ASCII);
                    assert_eq!(
                        allowed_ascii_prefix(&bytes),
                        scalar_prefix(&bytes, &ALLOWED_ASCII)
                    );
                    for class in &classes {
                        check(&bytes, class);
                        let expected = scalar_prefix(&bytes, class);
                        assert_eq!(class.prefix(&bytes), expected);
                        assert_eq!(class.short_prefix(&bytes), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn every_byte_ends_runs_like_scalar() {
        let classes = classes();
        for byte in 0..=255u8 {
            for at in [0, 15, 16, 31, 40, 63, 64, 100, 127] {
                let mut bytes = vec![b'a'; 130];
                bytes[at] = byte;
                check(&bytes, &ALLOWED_ASCII);
                for class in &classes {
                    check(&bytes, class);
                }
            }
        }
    }

    #[test]
    fn width_follows_level_and_length() {
        let widths = |levels: Levels| [0, 31, 32, 63, 64, 1000].map(|len| levels.choose(len).1);
        let fallback = Levels::new(Level::fallback());
        assert_eq!(widths(fallback), [Width::Scalar; 6]);
        let best = Level::new();
        let levels = Levels::new(best);
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use Width::{W16, W32, W64};
            if best.as_avx512().is_some() {
                assert_eq!(widths(levels), [W16, W16, W32, W32, W64, W64]);
            } else if best.as_avx2().is_some() {
                assert_eq!(widths(levels), [W16, W16, W32, W32, W32, W32]);
            } else {
                assert_eq!(widths(levels), [W16; 6]);
            }
            if let Some(simd) = best.as_sse4_2() {
                assert_eq!(widths(Levels::new(simd.level())), [W16; 6]);
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        assert_eq!(widths(levels), [Width::W16; 6]);
    }
}
