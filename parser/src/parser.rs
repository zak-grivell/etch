use ast::Expression;
use ast::Statement;

use crate::lexer::keyword;
use crate::lexer::{Keyword, Symbols, Token, symbol};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::HashMap;

type Span = SimpleSpan;
type Extra<'src> = extra::Err<Rich<'src, Token<'src>, Span>>;

pub fn parse<'src, I>() -> impl Parser<'src, I, Vec<Statement<'src>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let ident = select! {
        Token::Identifier(name) => name,
    };

    recursive(|body| {
        let expression = recursive(|expression| {
            // ok
            let atom = choice((
                select! {
                    Token::Number { value, prefix, unit } => Expression::Number { value, unit }, // TODO prefix
                    Token::Bool(val) => Expression::Boolean(val),
                    Token::Str(s) => Expression::String(s),
                },
                ident
                    .then_ignore(symbol!(Colon))
                    .then(expression.clone())
                    .separated_by(symbol!(Comma))
                    .allow_trailing()
                    .collect::<HashMap<_, _>>()
                    .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
                    .map(Expression::Object),
                expression
                    .clone()
                    .separated_by(symbol!(Comma))
                    .collect::<Vec<_>>()
                    .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
                    .map(Expression::Array),
                expression
                    .clone()
                    .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis)),
                ident
                    .then(symbol!(Colon).ignore_then(ident).or_not())
                    .separated_by(symbol!(Comma))
                    .collect::<HashMap<_, _>>()
                    .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                    .then_ignore(symbol!(Arrow))
                    .then(
                        body.clone()
                            .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly)),
                    )
                    .map(|(params, body)| Expression::Lambda { params, body }),
                body.clone()
                    .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly))
                    .map(|body| Expression::Block { body }),
                ident.map(Expression::Ident),
            ))
            .boxed();

            let dot = atom.foldl(symbol!(Dot).ignore_then(ident).repeated(), |expr, field| {
                Expression::ObjectAcess {
                    expr: Box::new(expr),
                    field,
                }
            });

            let call = dot
                .clone()
                .foldl(
                    ident
                        .then_ignore(symbol!(Colon))
                        .then(expression.clone())
                        .separated_by(symbol!(Comma))
                        .collect::<HashMap<_, _>>()
                        .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                        .repeated(),
                    |expr, args| Expression::Call {
                        expression: Box::new(expr),
                        args,
                    },
                )
                .boxed();

            let unary = choice((
                symbol!(Bang).to(Expression::Flip as fn(_) -> _),
                symbol!(Minus).to(Expression::Negate as fn(_) -> _),
            ))
            .repeated()
            .foldr(call.clone(), |op, rhs| op(Box::new(rhs)))
            .boxed();

            let products = unary
                .clone()
                .foldl(
                    choice((
                        symbol!(Slash).to(Expression::Div as fn(_, _) -> _),
                        symbol!(Star).to(Expression::Mul as fn(_, _) -> _),
                    ))
                    .then(unary)
                    .repeated(),
                    |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
                )
                .boxed();

            let sums = products
                .clone()
                .foldl(
                    choice((
                        symbol!(Plus).to(Expression::Add as fn(_, _) -> _),
                        symbol!(Minus).to(Expression::Sub as fn(_, _) -> _),
                    ))
                    .then(products)
                    .repeated(),
                    |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
                )
                .boxed();

            sums.clone().foldl(
                choice((
                    symbol!(Pipe).to(Expression::Union as fn(_, _) -> _),
                    symbol!(BackArrow).to(Expression::Wire as fn(_, _) -> _),
                ))
                .then(sums)
                .repeated(),
                |lhs, (op, rhs)| op(Box::new(lhs), Box::new(rhs)),
            )
        });

        let declaration = keyword!(Let)
            .ignore_then(ident)
            .then_ignore(symbol!(Equals))
            .then(expression.clone())
            .map(|(name, rhs)| Statement::Definition {
                name,
                rhs: Box::new(rhs),
            });

        let rtn = keyword!(Return)
            .ignore_then(expression.clone())
            .map(|expr| Statement::Return {
                value: Box::new(expr),
            });

        let statement = declaration
            .or(rtn)
            .or(expression.clone().map(Statement::Expression));

        statement
            .separated_by(symbol!(Semicolon))
            .allow_trailing()
            // .recover_with(skip_then_retry_until(any().ignored(), end()))
            // .repeated()
            .collect::<Vec<_>>()
    })
    .then_ignore(end())
}
