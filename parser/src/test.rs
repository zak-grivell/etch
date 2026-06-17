use std::fs::read_to_string;

use crate::lexer;
use crate::parser::parse;
use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::Parser as _;
use chumsky::error::Rich;
use chumsky::input::Input;
use chumsky::span::SimpleSpan;

fn print_errors<'src>(
    filename: &str,
    src: &'src str,
    errors: impl IntoIterator<Item = Rich<'src, String, SimpleSpan>>,
) {
    let filename: String = filename.into();

    for err in errors {
        let span = err.span().into_range();

        Report::build(ReportKind::Error, (filename.clone(), span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_message(err.to_string())
            .with_label(
                Label::new((filename.clone(), span))
                    .with_message(err.reason().to_string())
                    .with_color(Color::Red),
            )
            .with_labels(err.contexts().map(|(label, span)| {
                Label::new((filename.clone(), span.into_range()))
                    .with_message(format!("while parsing this {label}"))
                    .with_color(Color::Yellow)
            }))
            .finish()
            .print(sources([(filename.clone(), src)]))
            .unwrap();
    }
}

#[test]
pub fn test() {
    let input = read_to_string("../examples/main.etch").unwrap();
    let lex_luthor = lexer::lexer().parse(input.as_str());

    if let Some(tokens) = lex_luthor.output() {
        let eoi = SimpleSpan::from(input.len()..input.len());

        // dbg!(tokens);
        let parsed = parse().parse(tokens.as_slice().split_token_span(eoi));

        if let Some(output) = parsed.output() {
            dbg!(parsed);
        } else {
            print_errors(
                "filename",
                input.as_str(),
                parsed
                    .errors()
                    .map(|err| err.clone().map_token(|c| c.to_string())),
            );
        }
    } else {
        print_errors(
            "filename",
            input.as_str(),
            lex_luthor
                .errors()
                .map(|err| err.clone().map_token(|c| c.to_string())),
        );
    }
}
