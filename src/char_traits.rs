//! Character classification functions for YAML scanning.
//!
//! These functions classify characters according to the YAML specification,
//! including YAML 1.1 Unicode line break characters.

/// Check whether the character is nil (`\0`).
#[inline]
#[must_use]
pub fn is_z(c: char) -> bool {
    c == '\0'
}

/// Check whether the character is a line break.
///
/// Includes YAML 1.1 Unicode line breaks: NEL (`\u{0085}`), LS (`\u{2028}`), PS (`\u{2029}`).
#[inline]
#[must_use]
pub fn is_break(c: char) -> bool {
    matches!(c, '\r' | '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}

/// Check whether the character is nil or a line break.
#[inline]
#[must_use]
pub fn is_breakz(c: char) -> bool {
    is_break(c) || is_z(c)
}

/// Check whether the character is a YAML whitespace (` ` or `\t`).
#[inline]
#[must_use]
pub fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Check whether the character is nil, a line break, or a whitespace.
#[inline]
#[must_use]
pub fn is_blank_or_breakz(c: char) -> bool {
    is_blank(c) || is_breakz(c)
}

/// Check whether the character is ASCII alphanumeric, `_`, or `-`.
#[inline]
#[must_use]
pub fn is_alpha(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='z' | 'A'..='Z' | '_' | '-')
}

/// Check whether the character is an ASCII digit.
#[inline]
#[must_use]
pub fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Check whether the character is a hexadecimal digit (case insensitive).
#[inline]
#[must_use]
pub fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// Convert a hexadecimal digit to its integer value.
///
/// # Panics
///
/// Panics if `c` is not a valid hex digit.
#[inline]
#[must_use]
pub fn as_hex(c: char) -> u32 {
    c.to_digit(16).expect("not a hex digit")
}

/// Check whether the character is a YAML flow indicator (one of `,[]{}`).
#[inline]
#[must_use]
pub fn is_flow(c: char) -> bool {
    matches!(c, ',' | '[' | ']' | '{' | '}')
}

/// Check whether the character is the BOM character (`\u{FEFF}`).
#[inline]
#[must_use]
pub fn is_bom(c: char) -> bool {
    c == '\u{FEFF}'
}

/// Check whether the character is a printable YAML character.
#[inline]
#[must_use]
pub fn is_printable(c: char) -> bool {
    match c {
        '\u{feff}' | '\u{fffe}' | '\u{ffff}' => false,
        '\x0a'
        | '\x0d'
        | '\x20'..='\x7e'
        | '\u{0085}'
        | '\u{00a0}'..='\u{d7ff}'
        | '\u{e000}'..='\u{fffd}'
        | '\u{10000}'..='\u{10ffff}' => true,
        _ => false,
    }
}

/// Check whether the character is ASCII.
#[inline]
#[must_use]
pub fn is_ascii(c: char) -> bool {
    c.is_ascii()
}
