#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{Document, Parser, StrInput};

fuzz_target!(|data: &[u8]| fuzz_target(data));

fn fuzz_target(data: &[u8]) {
    let Ok(input) = core::str::from_utf8(data) else {
        return;
    };
    let mut parser = Parser::new(StrInput::new(input));

    while let Ok(mut document) = Document::load(&mut parser) {
        let done = document.get_root_node().is_none();
        if done {
            break;
        }
    }
}
