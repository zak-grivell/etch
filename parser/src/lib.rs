mod parser;

#[macro_use]
pub(crate) mod lexer;

pub(crate) mod cleaner;
pub(crate) mod resolver;
pub(crate) mod semantic;

#[cfg(test)]
mod test;

use ariadne::{Color, Label, Report, ReportKind, sources};
use ast::{AstNode, AstTransform};
use chumsky::Parser as _;
use chumsky::error::Rich;
use chumsky::input::IterInput;
use chumsky::span::{SimpleSpan, Span, Spanned};

use crate::cleaner::{CleanError, StrongMetadata};
use crate::lexer::Token;
use crate::resolver::{SymbolResolver, SymbolicError};
use crate::semantic::{SemanticError, TypeResolver};

pub enum CompileErrors<'src> {
    LexerError(Rich<'src, char, SimpleSpan>),
    ParserError(Rich<'src, Token<'src>, SimpleSpan>),
    ResolverError(Spanned<SymbolicError>),
    SemanticError(Spanned<SemanticError>),
    CleanerError(Spanned<CleanError>),
}

impl<'src> CompileErrors<'src> {
    fn stage(&self) -> String {
        match self {
            Self::LexerError(_) => String::from("lexer"),
            Self::ParserError(_) => String::from("parser"),
            Self::ResolverError(_) => String::from("symbol resolver"),
            Self::SemanticError(_) => String::from("type checking"),
            Self::CleanerError(_) => String::from("cleaner"),
        }
    }

    fn as_rich(self) -> Rich<'src, String, SimpleSpan> {
        match self {
            Self::LexerError(e) => e.map_token(|v| v.to_string()),
            Self::ParserError(e) => e.map_token(|v| v.to_string()),
            Self::ResolverError(e) => e.inner.into_rich(e.span),
            Self::SemanticError(e) => e.inner.into_rich(e.span),
            Self::CleanerError(e) => e.inner.into_rich(e.span),
        }
    }
}

pub fn compile<'a>(
    filename: &str,
    input: &'a str,
) -> Result<Vec<AstNode<StrongMetadata>>, Vec<CompileErrors<'a>>> {
    let (tokens, errors) = lexer::lexer().parse(input).into_output_errors();

    let lexer_errors = errors
        .into_iter()
        .map(|err| CompileErrors::LexerError(err))
        .collect();

    let Some(tokens) = tokens else {
        return Err(lexer_errors);
    };

    let eof = Span::new((), input.len()..input.len());

    let (parsed, errors) = parser::parse()
        .parse(IterInput::new(
            tokens
                .clone()
                .into_iter()
                .map(|t| (t.inner.clone(), t.span)),
            eof,
        ))
        .into_output_errors();

    let parser_errors: Vec<_> = errors
        .into_iter()
        .map(|err| CompileErrors::ParserError(err))
        .collect();

    let Some(parsed) = parsed else {
        return Err(parser_errors.into_iter().chain(lexer_errors).collect());
    };

    let (resolved, resolver_errors) = SymbolResolver::new().transform_all(parsed).into_parts();

    let resolver_errors = resolver_errors
        .into_iter()
        .map(|err| CompileErrors::ResolverError(err))
        .collect::<Vec<_>>();

    let (typed_tree, type_errors) = TypeResolver::new().transform_all(resolved).into_parts();

    let type_errors = type_errors
        .into_iter()
        .map(|err| CompileErrors::SemanticError(err))
        .collect::<Vec<_>>();

    let errors = [lexer_errors, parser_errors, resolver_errors, type_errors]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    if !errors.is_empty() {
        return Err(errors);
    }

    let (strong, errors) = cleaner::ExpressionStripper
        .transform_all(typed_tree)
        .into_parts();

    if !errors.is_empty() {
        return Err(errors
            .into_iter()
            .map(|v| CompileErrors::CleanerError(v))
            .collect());
    }

    Ok(strong)
}

pub fn print_errors(filename: &str, src: &str, errors: Vec<CompileErrors>) {
    let filename: String = filename.into();

    for err in errors {
        let stage = err.stage();
        let err = err.as_rich();
        let span = err.span().into_range();

        Report::build(ReportKind::Error, (filename.clone(), span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_message(err.to_string())
            .with_label(
                Label::new((filename.clone(), span))
                    .with_message(err.reason().to_string())
                    .with_color(Color::Red),
            )
            .with_labels(err.contexts().map(
                |(label, span)| -> Label<(String, std::ops::Range<usize>)> {
                    Label::new((filename.clone(), span.into_range()))
                        .with_message(format!("while parsing this {label} at state {stage}"))
                        .with_color(Color::Yellow)
                },
            ))
            .finish()
            .print(sources([(filename.clone(), src)]))
            .unwrap();
    }
}
