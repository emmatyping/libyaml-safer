#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{EventData, Parser};

fuzz_target!(|data: &[u8]| fuzz_target(data));

fn fuzz_target(mut data: &[u8]) {
    let mut parser = Parser::new();
    parser.set_input(&mut data);

    while let Ok(event) = parser.parse() {
        let is_end = matches!(event.data, EventData::StreamEnd);
        if is_end {
            break;
        }
    }
}
