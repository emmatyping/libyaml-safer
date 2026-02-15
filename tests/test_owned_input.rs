use libyaml_safer::{Document, EventData, Parser, Scanner, StrInput, TokenData};

#[test]
fn test_scanner_with_str_input() {
    let yaml = "key: value";
    let mut scanner = Scanner::new(StrInput::new(yaml));

    let token = scanner.next().unwrap().unwrap();
    assert!(matches!(token.data, TokenData::StreamStart { .. }));
}

#[test]
fn test_parser_with_str_input() {
    let yaml = "key: value\nlist:\n  - item1\n  - item2";
    let mut parser = Parser::new(StrInput::new(yaml));

    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::StreamStart { .. }));

    let event = parser.parse().unwrap();
    assert!(matches!(event.data, EventData::DocumentStart { .. }));
}

#[test]
fn test_document_load_with_str_input() {
    let yaml = r#"
users:
  - name: Alice
    age: 30
  - name: Bob
    age: 25
"#;

    let mut parser = Parser::new(StrInput::new(yaml));
    let doc = Document::load(&mut parser).unwrap();
    assert!(!doc.nodes.is_empty());
}

#[test]
fn test_parser_events() {
    const YAML: &str = "key: value";

    let mut parser = Parser::new(StrInput::new(YAML));

    let mut events = Vec::new();
    for event in &mut parser {
        events.push(format!("{:?}", event.unwrap().data));
    }

    assert!(!events.is_empty());
}
