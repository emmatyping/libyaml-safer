#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{Scanner, StrInput, TokenData};

fuzz_target!(|data: &[u8]| fuzz_target(data));

fn fuzz_target(data: &[u8]) {
    let Ok(input) = core::str::from_utf8(data) else {
        return;
    };
    let mut scanner = Scanner::new(StrInput::new(input));

    while let Ok(token) = Scanner::scan(&mut scanner) {
        let is_end = matches!(token.data, TokenData::StreamEnd);
        if is_end {
            break;
        }
    }
}
