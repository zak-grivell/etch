use chumsky::prelude::*;
use std::fmt::{self, Debug};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
    Bool(bool),
    Number {
        value: f64,
        prefix: Option<Prefix>,
        unit: Option<&'src str>,
    },
    Str(&'src str),

    Symbol(Symbols),
    Identifier(&'src str),
    Keyword(Keyword),
}

#[derive(Clone, Debug, PartialEq, Display, EnumString, EnumIter)]
pub enum Keyword {
    #[strum(serialize = "let")]
    Let,
    #[strum(serialize = "return")]
    Return,
    #[strum(serialize = "match")]
    Match,
}

#[derive(Clone, Debug, PartialEq, Display, EnumString, EnumIter)]
pub enum Symbols {
    #[strum(serialize = "==")]
    DoubleEquals,
    #[strum(serialize = "<")]
    Lt,
    #[strum(serialize = ">")]
    Gt,
    #[strum(serialize = "<=")]
    Le,
    #[strum(serialize = ">=")]
    Ge,
    #[strum(serialize = "!")]
    Bang,
    #[strum(serialize = "+")]
    Plus,
    #[strum(serialize = "-")]
    Minus,
    #[strum(serialize = "/")]
    Slash,
    #[strum(serialize = "*")]
    Star,
    #[strum(serialize = "|")]
    Pipe,
    #[strum(serialize = "<-")]
    BackArrow,
    #[strum(serialize = ";")]
    Semicolon,
    #[strum(serialize = ",")]
    Comma,
    #[strum(serialize = ".")]
    Dot,
    #[strum(serialize = "(")]
    OpenParenthesis,
    #[strum(serialize = ")")]
    ClosedParenthesis,
    #[strum(serialize = "[")]
    OpenSquare,
    #[strum(serialize = "]")]
    ClosedSquare,
    #[strum(serialize = "{")]
    OpenCurly,
    #[strum(serialize = "}}")]
    ClosedCurly,
    #[strum(serialize = "->")]
    Arrow,
    #[strum(serialize = ":")]
    Colon,
    #[strum(serialize = "=")]
    Equals,
}

#[derive(Clone, Debug, PartialEq, Display, EnumString, EnumIter)]
pub enum Prefix {
    #[strum(serialize = "n")]
    Nano,
    #[strum(serialize = "u")]
    Micro,
    #[strum(serialize = "m")]
    Mili,
    #[strum(serialize = "k")]
    Kilo,
    #[strum(serialize = "M")]
    Mega,
    #[strum(serialize = "G")]
    Giga,
}

fn enum_choice<'src, E>(
    mut values: Vec<E>,
) -> impl Parser<'src, &'src str, E, extra::Err<Rich<'src, char>>>
where
    E: Clone + ToString,
{
    values.sort_by_key(|v| std::cmp::Reverse(v.to_string().len()));

    choice(
        values
            .into_iter()
            .map(|v| just(v.to_string()).to(v))
            .collect::<Vec<_>>(),
    )
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::Bool(x) => write!(f, "{x}"),
            Token::Number {
                value,
                prefix,
                unit,
            } => {
                write!(f, "{value}")?;
                if let Some(prefix) = prefix {
                    write!(f, "{prefix}")?;
                }
                write!(f, "{}", unit.unwrap_or(""))
            }
            Token::Str(s) => write!(f, "{s}"),
            Token::Symbol(s) => write!(f, "{s}"),
            Token::Identifier(s) => write!(f, "{s}"),
            Token::Keyword(k) => write!(f, "{k}"),
        }
    }
}

pub fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<(Token<'src>, SimpleSpan)>, extra::Err<Rich<'src, char>>> {
    let num = text::int(10)
        .then(just('.').ignore_then(text::digits(10)).or_not())
        .to_slice()
        .from_str::<f64>()
        .unwrapped()
        .then(
            choice(
                Prefix::iter()
                    .map(|v| just(v.to_string()).to(v))
                    .collect::<Vec<_>>(),
            )
            .or_not(),
        )
        .then(text::ascii::ident().or_not())
        .map(|((value, prefix), unit)| Token::Number {
            value,
            prefix,
            unit,
        });

    let string = just('"')
        .ignore_then(none_of('"').repeated().to_slice())
        .then_ignore(just('"'))
        .map(Token::Str);

    let symbol = enum_choice(Symbols::iter().collect()).map(Token::Symbol);

    let keyword = enum_choice(Keyword::iter().collect()).map(Token::Keyword);

    let ident = text::ascii::ident().map(|ident: &str| match ident {
        "true" => Token::Bool(true),
        "false" => Token::Bool(false),
        _ => Token::Identifier(ident),
    });

    let token = choice((num, string, symbol, keyword, ident));

    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    token
        .map_with(|tok, e| {
            let span: SimpleSpan = e.span();
            (tok, span)
        })
        .padded_by(comment.repeated())
        .padded()
        // .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect::<Vec<_>>()
}

macro_rules! symbol {
    ($c:ident) => {
        just(Token::Symbol(Symbols::$c))
    };
}

macro_rules! keyword {
    ($c:ident) => {
        just(Token::Keyword(Keyword::$c))
    };
}

pub(crate) use keyword;
pub(crate) use symbol;
