use chumsky::{prelude::*, span::SpanWrap};
use std::fmt::{self, Debug};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
    Bool(bool),
    Number {
        value: f64,
        prefix: Option<Prefix>,
        unit: Option<&'src str>,
    },
    Str(String),

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
    #[strum(serialize = "type")]
    Type,

    #[strum(serialize = "from")]
    From,

    #[strum(serialize = "import")]
    Import,

    #[strum(serialize = "export")]
    Export,

    #[strum(serialize = "if")]
    If,
}

#[derive(Clone, Debug, PartialEq, EnumString, EnumIter, AsRefStr)]
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
    #[strum(serialize = "}")]
    ClosedCurly,
    #[strum(serialize = "->")]
    Arrow,
    #[strum(serialize = ":")]
    Colon,
    #[strum(serialize = "=")]
    Equals,

    #[strum(serialize = "@")]
    At,
    #[strum(serialize = "?")]
    Question,
}

impl fmt::Display for Symbols {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: &str = self.as_ref();
        write!(f, "{}", s)
    }
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

impl Prefix {
    fn multiplier(&self) -> f64 {
        match self {
            Self::Nano => 1e-9,
            Self::Micro => 1e-6,
            Self::Mili => 1e-3,
            Self::Kilo => 1e3,
            Self::Mega => 1e6,
            Self::Giga => 1e9,
        }
    }
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
                write!(
                    f,
                    "{}",
                    value / prefix.as_ref().map_or(1.0, Prefix::multiplier)
                )?;
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
-> impl Parser<'src, &'src str, Vec<Spanned<Token<'src>>>, extra::Err<Rich<'src, char>>> {
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
        .map(|((value, prefix), unit)| {
            let value = value * prefix.as_ref().map_or(1.0, Prefix::multiplier);
            Token::Number {
                value,
                prefix,
                unit,
            }
        })
        .labelled("Number");

    let escape = just('\\').ignore_then(choice((
        just('"').to('"'),
        just('\\').to('\\'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        just('0').to('\0'),
    )));
    let string = choice((escape, none_of("\\\"")))
        .repeated()
        .collect::<String>()
        .delimited_by(just('"'), just('"'))
        .map(Token::Str)
        .labelled("String");

    let symbol = enum_choice(Symbols::iter().collect())
        .map(Token::Symbol)
        .labelled("Symbol");

    let ident = text::ascii::ident()
        .map(|ident: &str| match ident {
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            "let" => Token::Keyword(Keyword::Let),
            "return" => Token::Keyword(Keyword::Return),
            "match" => Token::Keyword(Keyword::Match),
            "type" => Token::Keyword(Keyword::Type),
            "from" => Token::Keyword(Keyword::From),
            "import" => Token::Keyword(Keyword::Import),
            "export" => Token::Keyword(Keyword::Export),
            "if" => Token::Keyword(Keyword::If),
            _ => Token::Identifier(ident),
        })
        .labelled("Ident");

    let token = choice((num, string, symbol, ident));

    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    token
        .map_with(|tok, e| tok.with_span(e.span()))
        .padded_by(comment.repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
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
