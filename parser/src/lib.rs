mod parser;

#[macro_use]
pub(crate) mod lexer;

pub(crate) mod cleaner;
pub(crate) mod resolver;
pub(crate) mod semantic;

#[cfg(test)]
mod test;

use ariadne::{Color, Label, Report, ReportKind, sources};
use ast::AstNode;
use chumsky::Parser as _;
use chumsky::error::Rich;
use chumsky::input::IterInput;
use chumsky::span::{SimpleSpan, Span, Spanned};

use crate::lexer::Token;
use crate::parser::ParsedExpression;

pub fn lex<'a>(filename: &str, input: &'a str) -> Vec<Spanned<Token<'a>>> {
    let (output, errors) = lexer::lexer().parse(input).into_output_errors();

    print_errors(
        filename,
        input,
        errors
            .iter()
            .map(|e| e.clone().map_token(|c| c.to_string())),
        "lexer",
    );

    output.unwrap_or_default()
}

pub fn parse<'a>(
    filename: &str,
    input: &'a str,
    tokens: &'a [Spanned<Token<'a>>],
) -> Vec<AstNode<ParsedExpression>> {
    let eof = Span::new((), input.len()..input.len());

    let (output, errors) = parser::parse()
        .parse(IterInput::new(
            tokens.iter().map(|t| (t.inner.clone(), t.span)),
            eof,
        ))
        .into_output_errors();

    print_errors(
        filename,
        input,
        errors
            .iter()
            .map(|e| e.clone().map_token(|c| c.to_string())),
        "parser",
    );

    output.unwrap_or_default()
}

// pub fn infer(input: Vec<ParsedExpression>) -> Vec<PartiallyTypedExpression> {
//     input
//         .into_iter()
//         .map(|t| semantic::type_resolver(t, &TypeScope::default()))
//         .collect()
// }

fn print_errors<'src>(
    filename: &str,
    src: &'src str,
    errors: impl IntoIterator<Item = Rich<'src, String, SimpleSpan>>,
    stage: &str,
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
                    .with_message(format!("while parsing this {label} at state {stage}"))
                    .with_color(Color::Yellow)
            }))
            .finish()
            .print(sources([(filename.clone(), src)]))
            .unwrap();
    }
}
