pub(crate) fn is_alpha(ch: impl Into<Option<char>>) -> bool {
    let ch = match ch.into() {
        Some(ch) => ch,
        None => return false,
    };
    ch >= '0' && ch <= '9'
        || ch >= 'A' && ch <= 'Z'
        || ch >= 'a' && ch <= 'z'
        || ch == '_'
        || ch == '-'
}

pub(crate) fn is_ascii(ch: char) -> bool {
    ch.is_ascii()
}

pub(crate) fn is_printable(ch: char) -> bool {
    match ch {
        '\u{feff}' | '\u{fffe}' | '\u{ffff}' => false,
        '\x0a'
        | '\x20'..='\x7e'
        | '\u{00a0}'..='\u{00bf}'
        | '\u{00c0}'..='\u{cfff}'
        | '\u{d000}'..='\u{d7ff}'
        | '\u{e000}'..='\u{efff}'
        | '\u{f000}'..='\u{fffd}'
        | '\u{10000}'..='\u{10ffff}' => true,
        _ => false,
    }
}

pub(crate) fn is_bom(ch: char) -> bool {
    ch == '\u{feff}'
}

pub(crate) fn is_space(ch: impl Into<Option<char>>) -> bool {
    ch.into() == Some(' ')
}

pub(crate) fn is_blank(ch: impl Into<Option<char>>) -> bool {
    let ch = ch.into();
    is_space(ch) || ch == Some('\t')
}

pub(crate) fn is_break(ch: impl Into<Option<char>>) -> bool {
    matches!(
        ch.into(),
        Some('\r' | '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}')
    )
}

pub(crate) fn is_breakz(ch: impl Into<Option<char>>) -> bool {
    let ch = ch.into();
    ch.is_none() || is_break(ch)
}

pub(crate) fn is_blankz(ch: impl Into<Option<char>>) -> bool {
    let ch = ch.into();
    is_blank(ch) || is_breakz(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable() {
        for ch in "🎉".chars() {
            assert!(is_printable(ch));
        }
        for ch in "\u{1f389}".chars() {
            assert!(is_printable(ch));
        }
    }
}
