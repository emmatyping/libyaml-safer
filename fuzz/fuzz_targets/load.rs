#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{Document, Parser};

fuzz_target!(|data: &[u8]| fuzz_target(data));

fn fuzz_target(mut data: &[u8]) {
    let mut parser = Parser::new();
    parser.set_input(&mut data);

    while let Ok(mut document) = Document::load(&mut parser) {
        let done = document.get_root_node().is_none();
        if done {
            break;
        }
    }
}
