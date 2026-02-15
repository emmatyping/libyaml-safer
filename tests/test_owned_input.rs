use std::io::Cursor;

use libyaml_safer::{Document, EventData, Parser, Scanner, TokenData};

#[test]
fn test_scanner_with_owned_input() {
    let yaml_string = String::from("key: value");
    let mut scanner = Scanner::new();
    scanner.set_input(Cursor::new(yaml_string.into_bytes()));

    // Get the first token
    let token = scanner.next().unwrap().unwrap();
    assert!(matches!(token.data, TokenData::StreamStart { .. }));
}

#[test]
fn test_parser_with_owned_vec() {
    let yaml_data = b"key: value\nlist:\n  - item1\n  - item2".to_vec();
    let mut parser = Parser::new();
    parser.set_input(Cursor::new(yaml_data));

    // Parse the first event
    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::StreamStart { .. }));

    // Continue parsing to verify it works
    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::DocumentStart { .. }));
}

#[test]
fn test_parser_with_owned_string() {
    let yaml_string = String::from("name: test\nvalue: 123");
    let mut parser = Parser::new();
    parser.set_input(Cursor::new(yaml_string));

    // Parse the first event
    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::StreamStart { .. }));

    // Continue parsing to verify it works
    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::DocumentStart { .. }));
}

#[test]
fn test_document_load_with_owned_input() {
    let yaml_string = String::from(
        r#"
users:
  - name: Alice
    age: 30
  - name: Bob
    age: 25
"#,
    );

    let mut parser = Parser::new();
    parser.set_input(Cursor::new(yaml_string.into_bytes()));

    // Load the document
    let doc = Document::load(&mut parser).unwrap();

    // Verify we got a valid document with nodes
    assert!(!doc.nodes.is_empty());
}

#[test]
fn test_owned_input_no_lifetime_constraint() {
    // This test demonstrates that owned input doesn't require
    // the caller to maintain the buffer
    fn parse_yaml_owned() -> Parser<Cursor<String>> {
        let mut parser = Parser::new();
        let data = String::from("test: value");
        parser.set_input(Cursor::new(data));
        parser
    }

    let mut parser = parse_yaml_owned();
    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::StreamStart { .. }));
}

#[test]
fn test_owned_vs_borrowed_equivalence() {
    const YAML: &str = "key: value";

    // Parse with borrowed input
    let mut parser_borrowed = Parser::new();
    let mut borrowed_input = YAML.as_bytes();
    parser_borrowed.set_input_string(&mut borrowed_input);

    let mut events_borrowed = Vec::new();
    for event in &mut parser_borrowed {
        events_borrowed.push(format!("{:?}", event.unwrap().data));
    }

    // Parse with owned input
    let mut parser_owned = Parser::new();
    parser_owned.set_input(Cursor::new(String::from(YAML)));

    let mut events_owned = Vec::new();
    for event in &mut parser_owned {
        events_owned.push(format!("{:?}", event.unwrap().data));
    }

    // Verify both produce the same events
    assert_eq!(events_borrowed, events_owned);
}
