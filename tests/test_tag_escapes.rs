use std::io::Cursor;

use libyaml_safer::{EventData, INVALID_UTF8_ESCAPE, Parser};

/// The tag of the first node in `yaml`.
fn first_tag(yaml: &str) -> Option<String> {
    let mut parser = Parser::new();
    parser.set_input(Cursor::new(yaml.as_bytes().to_vec()));
    loop {
        match parser.parse().unwrap().data {
            EventData::Scalar { tag, .. }
            | EventData::SequenceStart { tag, .. }
            | EventData::MappingStart { tag, .. } => return tag,
            EventData::StreamEnd => panic!("no node in {yaml:?}"),
            _ => {}
        }
    }
}

#[test]
fn test_uri_escapes_decode_as_utf8() {
    assert_eq!(first_tag("!a%c3%a9 x").as_deref(), Some("!aé"));
    assert_eq!(first_tag("!a%E2%82%AC [x]").as_deref(), Some("!a€"));
    assert_eq!(
        first_tag("!<tag:%f0%9f%98%80> {}").as_deref(),
        Some("tag:😀")
    );
    assert_eq!(
        first_tag("%TAG !e! tag:%c3%a9,\n--- !e!%c3%a8 x").as_deref(),
        Some("tag:é,è")
    );
}

#[test]
fn test_uri_escapes_that_are_not_utf8_are_marked() {
    let marked = |s: &str| s.replace('|', &INVALID_UTF8_ESCAPE.to_string());
    // Overlong, surrogate and out-of-range sequences pass libyaml's checks.
    assert_eq!(first_tag("!a%c1%88 x"), Some(marked("!a|%C1%88")));
    assert_eq!(first_tag("!a%ed%a0%80b x"), Some(marked("!a|%ED%A0%80b")));
    assert_eq!(first_tag("!%f4%90%80%80 x"), Some(marked("!|%F4%90%80%80")));
    assert_eq!(
        first_tag("%TAG !e! tag:%e0%80%80\n--- !e!x%c3%a9 x"),
        Some(marked("tag:|%E0%80%80xé"))
    );
}

#[test]
fn test_uri_escaped_nul_ends_the_tag() {
    assert_eq!(first_tag("!a%00%c1%88 x").as_deref(), Some("!a"));
    assert_eq!(first_tag("!a%c3%a9%00b x").as_deref(), Some("!aé"));
    assert_eq!(first_tag("!%00- x").as_deref(), Some("!"));
    assert_eq!(
        first_tag("%TAG !e! tag:a%00b\n--- !e!x 1").as_deref(),
        Some("tag:ax")
    );
}
