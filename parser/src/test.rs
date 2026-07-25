use std::fs::read_to_string;

use crate::resolver::SymbolResolver;
use crate::semantic::TypeResolver;
use crate::{lex, parse};
use ariadne::{Label, Report, ReportKind, sources};
use ast::AstTransform;

#[test]
pub fn test() {
    let filename = "main.etch";

    let input = read_to_string("../examples/main.etch").unwrap();

    let tokens = lex("main.etch", &input);
    let parsed = parse("main.etch", &input, &tokens);
    let (symols, errors) = SymbolResolver::new().transform_all(parsed).into_parts();

    for err in errors.iter() {
        let span = err.span.into_range();

        Report::build(ReportKind::Error, (filename.clone(), span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_message(err.inner.to_string())
            .with_label(Label::new((filename.clone(), span)))
            .finish()
            .print(sources([(filename.clone(), &input)]))
            .unwrap();
    }

    let (types, ty_errors) = TypeResolver::new().transform_all(symols).into_parts();

    for err in ty_errors.iter() {
        let span = err.span.into_range();
        Report::build(ReportKind::Error, (filename.clone(), span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_message(err.inner.to_string())
            .with_label(Label::new((filename.clone(), span)))
            .finish()
            .print(sources([(filename.clone(), &input)]))
            .unwrap();
    }

    println!("{types:?}");
}
