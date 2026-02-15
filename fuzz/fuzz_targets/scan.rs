#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{Scanner, TokenData};

fuzz_target!(|data: &[u8]| fuzz_target(data));

fn fuzz_target(mut data: &[u8]) {
    let mut scanner = Scanner::new();
    scanner.set_input(&mut data);

    while let Ok(token) = Scanner::scan(&mut scanner) {
        let is_end = matches!(token.data, TokenData::StreamEnd);
        if is_end {
            break;
        }
    }
}
