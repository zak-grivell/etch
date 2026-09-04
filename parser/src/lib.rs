pub mod parser;

#[macro_use]
pub(crate) mod lexer;

pub(crate) mod cleaner;
pub(crate) mod resolver;
pub(crate) mod semantic;

pub use resolver::Symbol;
pub use semantic::{PartialMetadata, TypedProgram, ValueType};

#[cfg(test)]
mod test;

use ariadne::{Color, Label, Report, ReportKind, sources};
use ast::{AstNode, AstTraverse, Program};
use chumsky::Parser as _;
use chumsky::error::Rich;
use chumsky::input::IterInput;
use chumsky::span::{SimpleSpan, Span};

use crate::lexer::Token;
use crate::resolver::{ResolverError, SymbolResolver};
use crate::semantic::{SemanticError, TypeResolver};

pub enum CompileErrors<'src> {
    LexerError(Rich<'src, char, SimpleSpan>),
    ParserError(Rich<'src, Token<'src>, SimpleSpan>),
    ResolverError(ResolverError),
    SemanticError(SemanticError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileDiagnostic {
    pub stage: &'static str,
    pub message: String,
    pub span: std::ops::Range<usize>,
}

impl<'src> CompileErrors<'src> {
    fn stage(&self) -> &'static str {
        match self {
            Self::LexerError(_) => "lexer",
            Self::ParserError(_) => "parser",
            Self::ResolverError(_) => "symbol resolver",
            Self::SemanticError(_) => "type checking",
        }
    }

    pub fn diagnostic(&self) -> CompileDiagnostic {
        match self {
            Self::LexerError(error) => CompileDiagnostic {
                stage: self.stage(),
                message: error.to_string(),
                span: error.span().into_range(),
            },
            Self::ParserError(error) => CompileDiagnostic {
                stage: self.stage(),
                message: error.to_string(),
                span: error.span().into_range(),
            },
            Self::ResolverError(error) => CompileDiagnostic {
                stage: self.stage(),
                message: error.message.clone(),
                span: error.span.into_range(),
            },
            Self::SemanticError(error) => CompileDiagnostic {
                stage: self.stage(),
                message: error.message.clone(),
                span: error.span.into_range(),
            },
        }
    }

    fn into_rich(self) -> Rich<'src, String, SimpleSpan> {
        match self {
            Self::LexerError(e) => e.map_token(|v| v.to_string()),
            Self::ParserError(e) => e.map_token(|v| v.to_string()),
            Self::ResolverError(e) => Rich::custom(e.span, e.message),
            Self::SemanticError(e) => Rich::custom(e.span, e.message),
        }
    }
}

pub type ParsedProgram = AstNode<Program<parser::ParsedNode>, parser::ParsedNode>;

pub fn compile<'a>(input: &'a str) -> Result<Vec<TypedProgram>, Vec<CompileErrors<'a>>> {
    let (tokens, errors) = lexer::lexer().parse(input).into_output_errors();

    let lexer_errors: Vec<_> = errors.into_iter().map(CompileErrors::LexerError).collect();

    let Some(tokens) = tokens else {
        return Err(lexer_errors);
    };

    let eof = Span::new((), input.len()..input.len());

    let (parsed, errors) = parser::parse()
        .parse(IterInput::new(
            tokens.into_iter().map(|t| (t.inner, t.span)),
            eof,
        ))
        .into_output_errors();

    let parser_errors: Vec<_> = errors.into_iter().map(CompileErrors::ParserError).collect();
    let errors = lexer_errors
        .into_iter()
        .chain(parser_errors)
        .collect::<Vec<_>>();

    if !errors.is_empty() {
        return Err(errors);
    }

    let Some(parsed) = parsed else {
        return Err(Vec::new());
    };

    let (resolved, resolver_errors) = SymbolResolver::new().transform_all(parsed).into_parts();
    let resolver_errors = resolver_errors
        .into_iter()
        .map(CompileErrors::ResolverError)
        .collect::<Vec<_>>();
    if !resolver_errors.is_empty() {
        return Err(resolver_errors);
    }

    let (typed, semantic_errors) = TypeResolver::new().transform_all(resolved).into_parts();
    let semantic_errors = semantic_errors
        .into_iter()
        .map(CompileErrors::SemanticError)
        .collect::<Vec<_>>();
    if !semantic_errors.is_empty() {
        return Err(semantic_errors);
    }

    let (cleaned, cleaner_errors) = cleaner::ExpressionStripper
        .transform_all(typed)
        .into_parts();
    debug_assert!(cleaner_errors.is_empty());

    Ok(cleaned)
}

pub fn print_errors(filename: &str, src: &str, errors: Vec<CompileErrors>) {
    let filename: String = filename.into();

    for err in errors {
        let stage = err.stage();
        let err = err.into_rich();
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
