use std::fs::read_to_string;

use crate::compile;
use crate::lexer::{Prefix, Token, lexer};
use chumsky::Parser;

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
fn parses_exported_top_level_bindings() {
    let source = "export let component = (value: Number) -> value";
    let programs = compile(source).unwrap_or_else(|errors| {
        crate::print_errors("test.etch", source, errors);
        panic!("export should compile")
    });
    assert!(matches!(programs[0].inner, ast::Program::Export(_)));
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

#[test]
fn applies_si_prefixes_and_preserves_units() {
    let (tokens, errors) = lexer().parse("1kOhm").into_output_errors();
    assert!(errors.is_empty());
    assert_eq!(
        tokens.unwrap()[0].inner,
        Token::Number {
            value: 1000.0,
            prefix: Some(Prefix::Kilo),
            unit: Some("Ohm"),
        }
    );
}

#[test]
fn parses_and_checks_comparisons() {
    assert!(compile("1 < 2; 2 >= 1; true == false").is_ok());
    assert!(compile("true < false").is_err());
}

#[test]
fn supports_object_and_pattern_shorthand() {
    assert!(compile("let value = 1; let { value } = { value }").is_ok());
}
