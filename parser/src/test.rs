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
    let input = read_to_string("../examples/voltage_divider.etch").unwrap();

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
fn checks_user_defined_numeric_units() {
    let valid = r#"
        type Voltage = Number(symbol: "V");
        type Resistance = Number(symbol: "Ω");
        let supply = (value: Voltage) -> value;
        let resistor = (value: Resistance) -> value;
        supply(value: 5);
        resistor(value: 1000)
    "#;
    assert!(compile(valid).is_ok());

    let invalid = r#"
        type Voltage = Number(symbol: "V");
        type Resistance = Number(symbol: "Ω");
        let supply = (value: Voltage) -> value;
        let resistor = (value: Resistance) -> value;
        supply(value: resistor(value: 1000))
    "#;
    assert!(compile(invalid).is_err());
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

#[test]
fn supports_interleaved_calls_and_field_access() {
    assert!(
        compile("let make = () -> { value: () -> { answer: 42 } }; make().value().answer").is_ok()
    );
}

#[test]
fn function_type_parameters_do_not_leak_into_values() {
    assert!(compile("type Callback = (argument: Number) -> Number; argument").is_err());
}

#[test]
fn object_merges_have_merged_field_types() {
    assert!(compile("let merged = { a: 1 } | { b: true }; merged.a + 1").is_ok());
    assert!(compile("1 | 2").is_err());
}

#[test]
fn numeric_aliases_obey_block_scope() {
    assert!(compile(r#"{ type Hidden = Number; 0 }; let f = (x: Hidden) -> x"#).is_err());
}

#[test]
fn checks_match_bindings_and_early_return_types() {
    assert!(compile("match 1 { let x -> x + true }").is_err());
    assert!(compile("let f = () -> { return 1; true }; f() + 1").is_ok());
}

#[test]
fn shadowing_initializers_resolve_the_previous_binding() {
    assert!(compile("let x = 1; let x = x + 1; x").is_ok());
    assert!(compile("let x = x + 1").is_err());
}

#[test]
fn aliases_cover_structural_optional_union_and_function_types() {
    for source in [
        "type Item = { a: Number }; let f = (x: Item) -> x.a; f(x: { a: 2, extra: true })",
        "type Choice = Number | Bool; let f = (x: Choice) -> x; f(x: true)",
        "type Maybe = Number?; let f = (x: Maybe) -> x; f(x: 2)",
        "type Items = [Number]; let f = (x: Items) -> x; f(x: [1,2])",
        "type Callback = (a: Number) -> Number; let f = (x: Callback) -> x(a: 2); f(x: (a: Number) -> a + 1)",
    ] {
        assert!(compile(source).is_ok(), "{source}");
    }
    assert!(
        compile("type Item = { a: Number }; let f = (x: Item) -> x; f(x: { a: true })").is_err()
    );
    assert!(compile("1V + 1A").is_err());
    assert!(compile(r#""bad\q""#).is_err());
}

#[test]
fn tuple_array_and_optional_assignability_is_structural() {
    assert!(compile("type Pair = (Number, Bool); let f = (x: Pair) -> x; f(x: [1,true])").is_ok());
    assert!(compile("type Pair = (Number, Bool); let f = (x: Pair) -> x; f(x: [true,1])").is_err());
    assert!(compile("let f = (x: [Number]) -> x; f(x: [1,true])").is_err());
    assert!(compile("let f = (x: [Number]) -> x; f(x: [])").is_ok());
    assert!(compile("let f = (x: Number?) -> x; f(x: { let n = 1 })").is_ok());
    assert!(compile("type Foo = Number; Foo").is_err());
}
