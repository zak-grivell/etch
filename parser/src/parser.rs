use ast::{
    Ast, AstNode, BinaryOperator, Expression, Ident, PartialType, Span, Type, UnaryOperator,
};

use crate::lexer::keyword;
use crate::lexer::{Keyword, Symbols, Token, symbol};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::BTreeMap;

type Extra<'src> = extra::Err<Rich<'src, Token<'src>, Span>>;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedExpression {
    pub expression: Expression<ParsedExpression>,
}

impl Ast for ParsedExpression {
    type E = AstNode<ParsedExpression>;
    type T = PartialType;
    type I = String;

    type B = BinaryOperator;
    type U = UnaryOperator;

    fn expression(self) -> Expression<Self> {
        self.expression
    }
}

pub fn parse<'src, I>() -> impl Parser<'src, I, Vec<AstNode<ParsedExpression>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let ident = select! {
        Token::Identifier(name) => name.to_string(),
    };

    recursive(|body| {
        recursive(|expression| {
            let atom = choice((
                select! {
                    Token::Number { value, unit, .. } => Expression::new_number(
                        value, unit.map(|v| v.to_string() )),
                    Token::Bool(val) => Expression::new_boolean(val) ,
                    Token::Str(s) => Expression::new_string(s.to_string()) ,
                },
                ident
                    .then_ignore(symbol!(Colon))
                    .then(expression.clone())
                    .separated_by(symbol!(Comma))
                    .allow_trailing()
                    .collect::<BTreeMap<_, _>>()
                    .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
                    .map(Expression::new_object),
                expression
                    .clone()
                    .separated_by(symbol!(Comma))
                    .collect::<Vec<_>>()
                    .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
                    .map(Expression::new_array),
                ident
                    .then(symbol!(Colon).ignore_then(ident))
                    .separated_by(symbol!(Comma))
                    .collect::<BTreeMap<_, _>>()
                    .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                    .then_ignore(symbol!(Arrow))
                    .then(
                        body.clone()
                            .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly)),
                    )
                    .map(|(params, body)| {
                        let typed_params = params
                            .into_iter()
                            .map(|(k, v)| {
                                (
                                    k,
                                    PartialType::T(match v.as_str() {
                                        "number" => Type::Number,
                                        "string" => Type::String,
                                        "bool" => Type::Boolean,
                                        "void" => Type::Void,
                                        s => Type::Wrapper {
                                            name: s.to_string(),
                                            inner: Box::new(PartialType::T(Type::Number)),
                                        },
                                    }),
                                )
                            })
                            .collect();

                        Expression::new_lambda(typed_params, body)
                    }),
                ident.map(|value| Ident { value }).map(Expression::Ident),
            ))
            .spanned()
            .map(|spanned_expression: Spanned<_, SimpleSpan>| {
                AstNode::new(
                    ParsedExpression {
                        expression: spanned_expression.inner,
                    },
                    spanned_expression.span,
                )
            })
            .boxed();

            let tree = choice((
                atom,
                expression
                    .clone()
                    .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis)),
            ))
            .boxed();

            let dot = tree
                .foldl_with(
                    symbol!(Dot).ignore_then(ident).repeated(),
                    |expr: AstNode<ParsedExpression>, field, e| {
                        AstNode::new(
                            ParsedExpression {
                                expression: Expression::<ParsedExpression>::new_object_access(
                                    expr, field,
                                ),
                            },
                            e.span(),
                        )
                    },
                )
                .boxed();

            let call = dot
                .foldl_with(
                    ident
                        .then_ignore(symbol!(Colon))
                        .then(expression.clone())
                        .separated_by(symbol!(Comma))
                        .collect::<BTreeMap<_, _>>()
                        .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                        .repeated(),
                    |expr, args, e| {
                        AstNode::new(
                            ParsedExpression {
                                expression: Expression::new_call(expr, args),
                            },
                            e.span(),
                        )
                    },
                )
                .boxed();

            let unary = choice((
                symbol!(Bang).to(UnaryOperator::Flip),
                symbol!(Minus).to(UnaryOperator::Negate),
            ))
            .repeated()
            .foldr_with(call, |operation, expression, e| {
                AstNode::new(
                    ParsedExpression {
                        expression: Expression::new_unary_operation(expression, operation),
                    },
                    e.span(),
                )
            })
            .boxed();

            let products = unary
                .clone()
                .foldl_with(
                    choice((
                        symbol!(Slash).to(BinaryOperator::Div),
                        symbol!(Star).to(BinaryOperator::Mul),
                    ))
                    .then(unary)
                    .repeated(),
                    |lhs, (op, rhs), e| {
                        AstNode::new(
                            ParsedExpression {
                                expression: Expression::new_binary_operation(lhs, rhs, op),
                            },
                            e.span(),
                        )
                    },
                )
                .boxed();

            let sums = products
                .clone()
                .foldl_with(
                    choice((
                        symbol!(Plus).to(BinaryOperator::Add),
                        symbol!(Minus).to(BinaryOperator::Sub),
                    ))
                    .then(products)
                    .repeated(),
                    |lhs, (op, rhs), e| {
                        AstNode::new(
                            ParsedExpression {
                                expression: Expression::new_binary_operation(lhs, rhs, op),
                            },
                            e.span(),
                        )
                    },
                )
                .boxed();

            let sums = sums
                .clone()
                .foldl_with(
                    choice((
                        symbol!(Pipe).to(BinaryOperator::Union),
                        symbol!(BackArrow).to(BinaryOperator::Wire),
                    ))
                    .then(sums)
                    .repeated(),
                    |lhs, (op, rhs), e| {
                        AstNode::new(
                            ParsedExpression {
                                expression: Expression::new_binary_operation(lhs, rhs, op),
                            },
                            e.span(),
                        )
                    },
                )
                .boxed();

            let declaration = keyword!(Let)
                .ignore_then(ident)
                .then_ignore(symbol!(Equals))
                .then(expression.clone())
                .map_with(|(name, rhs), e| {
                    AstNode::new(
                        ParsedExpression {
                            expression: Expression::new_definition(name, rhs),
                        },
                        e.span(),
                    )
                })
                .boxed();

            let rtn = keyword!(Return)
                .ignore_then(expression.clone())
                .map_with(|expr, e| {
                    AstNode::new(
                        ParsedExpression {
                            expression: Expression::new_return(expr),
                        },
                        e.span(),
                    )
                })
                .boxed();

            choice((rtn, declaration, sums))
        })
        .separated_by(symbol!(Semicolon))
        .allow_trailing()
        .collect::<Vec<_>>()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
    })
    .then_ignore(end())
}
