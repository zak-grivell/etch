use std::fs::read_to_string;

use crate::compile;

#[test]
fn parses_example_program() {
    let input = "1".to_string();

    assert!(compile(&input).is_ok());
}

#[test]
fn parses_the_example_program() {
    let input = read_to_string("../examples/main.etch").unwrap();

    assert!(compile(&input).is_ok());
}

#[test]
fn keeps_keywords_inside_identifiers() {
    assert!(compile("let returnValue = 1").is_ok());
}

#[test]
fn rejects_duplicate_named_fields() {
    assert!(compile("f(value: 1, value: 2)").is_err());
}

#[test]
fn parses_all_supported_type_forms() {
    let input = "type Callback = (value: Number) -> String; type Value = Number? | String; type Item = { value: Number }";

    assert!(compile(input).is_ok());
}

#[test]
fn rejects_match_arms_without_a_pattern_or_guard() {
    assert!(compile("match value { => 1 }").is_err());
}

#[test]
fn reports_undefined_names() {
    assert!(compile("missing").is_err());
}

#[test]
fn reports_invalid_operations() {
    assert!(compile("true + 1").is_err());
}
