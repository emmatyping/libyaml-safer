use libyaml_safer::Parser;

/// Test that errors at the very beginning display as line 1 column 1.
#[test]
fn first_position() {
    const INVALID_YAML: &str = "\t";

    let mut parser = Parser::new();
    let mut input = INVALID_YAML.as_bytes();
    parser.set_input_string(&mut input);

    let result = parser.collect::<Result<Vec<_>, _>>();

    assert!(result.is_err(), "Expected parsing to fail for invalid YAML");
    let err = result.unwrap_err();

    let mark = err.problem_mark().unwrap();
    let mark_str = mark.to_string();
    eprintln!("Problem mark: {}", mark_str);

    assert_eq!(mark_str, "line 1 column 1");
}

/// Test that error messages display 1-based line and column numbers.
///
/// This YAML has a missing closing quote (from test CQ3W).
#[test]
fn multiline_error() {
    const INVALID_YAML: &str = "---\nkey: \"missing closing quote";

    let mut parser = Parser::new();
    let mut input = INVALID_YAML.as_bytes();
    parser.set_input_string(&mut input);

    let result = parser.collect::<Result<Vec<_>, _>>();

    assert!(result.is_err(), "Expected parsing to fail for invalid YAML");
    let err = result.unwrap_err();

    // Verify that the mark can be retrieved and displays correctly
    let mark = err.problem_mark().unwrap();
    let mark_str = mark.to_string();
    eprintln!("Problem mark: {}", mark_str);

    assert_eq!(mark_str, "line 2 column 28");
}
