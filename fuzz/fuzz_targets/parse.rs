#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{EventData, Parser, StrInput};

fuzz_target!(|data: &[u8]| fuzz_target(data));

fn fuzz_target(data: &[u8]) {
    let Ok(input) = core::str::from_utf8(data) else {
        return;
    };
    let mut parser = Parser::new(StrInput::new(input));

    while let Ok(event) = parser.parse() {
        let is_end = matches!(event.data, EventData::StreamEnd);
        if is_end {
            break;
        }
    }
}
