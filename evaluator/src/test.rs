use std::fs::read_to_string;

use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::Parser as _;
use chumsky::error::Rich;
use chumsky::input::Input;
use chumsky::span::SimpleSpan;
use parser::{lex, parse};

use crate::{Scope, evaluate_statements};

#[test]
pub fn eval() {
    let input = read_to_string("../examples/main.etch").unwrap();

    let tokens = lex("main.etch", &input);
    let parser = parse("main.etch", &input, &tokens);

    dbg!(&parser);
    let scope = Scope::default();
    let e = evaluate_statements(&parser, &scope);

    println!("{:?} {:?}", e, scope)
}
