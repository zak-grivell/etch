use ast::{
    Ast, AstNode, BinaryOperator, Expression, Ident, Import, Pattern, Primative, Program, Span,
    Statement, Type, UnaryOperator,
};
use chumsky::extra::ParserExtra;

use crate::lexer::keyword;
use crate::lexer::{Keyword, Symbols, Token, symbol};
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::BTreeMap;

type Extra<'src> = extra::Err<Rich<'src, Token<'src>, Span>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParsedMetadata {
    pub span: Span,
}

type Parsed<T> = AstNode<T, ParsedMetadata>;

impl Ast for ParsedMetadata {
    type Node<T> = AstNode<T, Self>;

    type Expression = Self::Node<Expression<ParsedMetadata>>;
    type Pattern = Self::Node<Pattern<ParsedMetadata>>;
    type Type = Self::Node<Type<ParsedMetadata>>;
    type Statement = Self::Node<Statement<ParsedMetadata>>;

    type Ident = String;
    type B = BinaryOperator;
    type U = UnaryOperator;
}

pub trait SpannedExt<'src, I, O, E>: Parser<'src, I, O, E> + Sized
where
    I: Input<'src, Span = SimpleSpan>,
    E: ParserExtra<'src, I>,
{
    fn spanned_node(self) -> impl Parser<'src, I, AstNode<O, ParsedMetadata>, E> {
        self.map_with(|inner, e| AstNode {
            inner,
            meta: ParsedMetadata { span: e.span() },
        })
    }
}

impl<'src, I, O, E, P> SpannedExt<'src, I, O, E> for P
where
    P: Parser<'src, I, O, E>,
    I: Input<'src, Span = Span>,
    E: ParserExtra<'src, I>,
{
}

pub fn program_parse<'src, I>()
-> impl Parser<'src, I, Vec<Parsed<Program<ParsedMetadata>>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let import = keyword!(From)
        .ignore_then(select! {
            Token::Str(str) => str
        })
        .then_ignore(keyword!(Import))
        .then(pattern_parse())
        .map(|(path, imports)| Import {
            imports,
            path: path.to_string(),
        })
        .map(Program::Import)
        .boxed()
        .spanned_node();

    let s = statement_parse()
        .map(|st| st.map(Program::Statement))
        .boxed();

    choice((import, s))
        .separated_by(symbol!(Semicolon))
        .allow_trailing()
        .collect::<Vec<_>>()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
}

pub fn primative_parse<'src, I>() -> impl Parser<'src, I, Parsed<Primative>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    select! {
        Token::Number { value, .. } => Primative::Number(value),
        Token::Bool(val) => Primative::Boolean(val),
        Token::Str(s) => Primative::String(s.to_string()) ,
    }
    .boxed()
    .spanned_node()
}

pub fn statement_parse<'src, I>()
-> impl Parser<'src, I, Vec<Parsed<Statement<ParsedMetadata>>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let defintion = keyword!(Let)
        .ignore_then(pattern_parse())
        .then_ignore(symbol!(Equals))
        .then(expression_parse())
        .map(|(lhs, rhs)| ast::Definition { lhs, rhs })
        .map(Statement::Definition)
        .boxed()
        .spanned_node();

    let type_definition = keyword!(Type)
        .ignore_then(select! {
            Token::Identifier(ident) => ident
        })
        .then_ignore(symbol!(Equals))
        .then(type_parse())
        .map(|(lhs, rhs)| ast::TypeDefinition {
            lhs: lhs.to_string(),
            rhs,
        })
        .map(Statement::TypeDefinition)
        .boxed()
        .spanned_node();

    let rtn = keyword!(Return)
        .ignore_then(expression_parse())
        .map(|value| ast::Return { value })
        .map(Statement::Return)
        .boxed()
        .spanned_node();

    let expression = expression_parse()
        .map(|st| st.map(Statement::Expression))
        .boxed();

    choice((defintion, type_definition, rtn, expression))
        .separated_by(symbol!(Semicolon))
        .allow_trailing()
        .collect::<Vec<_>>()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
}

pub fn expression_parse<'src, I>()
-> impl Parser<'src, I, Parsed<Expression<ParsedMetadata>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let primative = primative_parse()
        .map(|st| st.map(Expression::Primative))
        .boxed();

    choice((primative))
}

pub fn pattern_parse<'src, I>() -> impl Parser<'src, I, Parsed<Pattern<ParsedMetadata>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let primative = primative_parse().map(|st| st.map(Pattern::Primative));

    todo!();
}

pub fn type_parse<'src, I>() -> impl Parser<'src, I, Parsed<Type<ParsedMetadata>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    todo!();
}

// pub fn parse<'src, I>() -> impl Parser<'src, I, AstNode<ParsedMetadata>, Extra<'src>>
// where
//     I: ValueInput<'src, Token = Token<'src>, Span = Span>,
// {
//     let ident = select! {
//         Token::Identifier(name) => name.to_string(),
//     };

//     recursive(|body| {
//         recursive(|expression| {
//             let atom = choice((
// select! {
//     Token::Number { value, unit, .. } => Expression::new_number(
//         value, unit.map(|v| v.to_string() )),
//     Token::Bool(val) => Expression::new_boolean(val) ,
//     Token::Str(s) => Expression::new_string(s.to_string()) ,
// },
//                 symbol!(At).map(|_| Expression::new_node()),
//                 ident
//                     .then_ignore(symbol!(Colon))
//                     .then(expression.clone())
//                     .separated_by(symbol!(Comma))
//                     .allow_trailing()
//                     .collect::<BTreeMap<_, _>>()
//                     .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
//                     .map(Expression::new_object),
//                 expression
//                     .clone()
//                     .separated_by(symbol!(Comma))
//                     .collect::<Vec<_>>()
//                     .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
//                     .map(Expression::new_array),
//                 ident
//                     .then(symbol!(Colon).ignore_then(ident))
//                     .separated_by(symbol!(Comma))
//                     .collect::<BTreeMap<_, _>>()
//                     .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
//                     .then_ignore(symbol!(Arrow))
//                     .then(
//                         body.clone()
//                             .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly)),
//                     )
//                     .map(|(params, body)| {
//                         let typed_params = params
//                             .into_iter()
//                             .map(|(k, v)| {
//                                 (
//                                     k,
//                                     PartialType::T(match v.as_str() {
//                                         "number" => Type::Number,
//                                         "string" => Type::String,
//                                         "bool" => Type::Boolean,
//                                         "void" => Type::Void,
//                                         s => Type::Wrapper {
//                                             name: s.to_string(),
//                                             inner: Box::new(PartialType::T(Type::Number)),
//                                         },
//                                     }),
//                                 )
//                             })
//                             .collect();

//                         Expression::new_lambda(typed_params, body)
//                     }),
//                 ident
//                     .map(|value| Ident { ident: value })
//                     .map(Expression::Ident),
//             ))
//             .spanned()
//             .map(|spanned_expression: Spanned<_, SimpleSpan>| {
//                 AstNode::new(
//                     spanned_expression.inner,
//                     ParsedMetadata {
//                         span: spanned_expression.span,
//                     },
//                 )
//             })
//             .boxed();

//             let tree = choice((
//                 atom,
//                 expression
//                     .clone()
//                     .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis)),
//             ))
//             .boxed();

//             let dot = tree
//                 .foldl_with(
//                     symbol!(Dot).ignore_then(ident).repeated(),
//                     |expr: AstNode<ParsedMetadata>, field, e| {
//                         AstNode::new(
//                             Expression::<ParsedMetadata>::new_object_access(expr, field),
//                             ParsedMetadata { span: e.span() },
//                         )
//                     },
//                 )
//                 .boxed();

//             let call = dot
//                 .foldl_with(
//                     ident
//                         .then_ignore(symbol!(Colon))
//                         .then(expression.clone())
//                         .separated_by(symbol!(Comma))
//                         .collect::<BTreeMap<_, _>>()
//                         .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
//                         .repeated(),
//                     |expr, args, e| {
//                         AstNode::new(
//                             Expression::new_call(expr, args),
//                             ParsedMetadata { span: e.span() },
//                         )
//                     },
//                 )
//                 .boxed();

//             let unary = choice((
//                 symbol!(Bang).to(UnaryOperator::Flip),
//                 symbol!(Minus).to(UnaryOperator::Negate),
//             ))
//             .repeated()
//             .foldr_with(call, |operation, expression, e| {
//                 AstNode::new(
//                     Expression::new_unary_operation(expression, operation),
//                     ParsedMetadata { span: e.span() },
//                 )
//             })
//             .boxed();

//             let products = unary
//                 .clone()
//                 .foldl_with(
//                     choice((
//                         symbol!(Slash).to(BinaryOperator::Div),
//                         symbol!(Star).to(BinaryOperator::Mul),
//                     ))
//                     .then(unary)
//                     .repeated(),
//                     |lhs, (op, rhs), e| {
//                         AstNode::new(
//                             Expression::new_binary_operation(lhs, rhs, op),
//                             ParsedMetadata { span: e.span() },
//                         )
//                     },
//                 )
//                 .boxed();

//             let sums = products
//                 .clone()
//                 .foldl_with(
//                     choice((
//                         symbol!(Plus).to(BinaryOperator::Add),
//                         symbol!(Minus).to(BinaryOperator::Sub),
//                     ))
//                     .then(products)
//                     .repeated(),
//                     |lhs, (op, rhs), e| {
//                         AstNode::new(
//                             Expression::new_binary_operation(lhs, rhs, op),
//                             ParsedMetadata { span: e.span() },
//                         )
//                     },
//                 )
//                 .boxed();

//             let sums = sums
//                 .clone()
//                 .foldl_with(
//                     choice((
//                         symbol!(Pipe).to(BinaryOperator::Union),
//                         symbol!(BackArrow).to(BinaryOperator::Wire),
//                     ))
//                     .then(sums)
//                     .repeated(),
//                     |lhs, (op, rhs), e| {
//                         AstNode::new(
//                             Expression::new_binary_operation(lhs, rhs, op),
//                             ParsedMetadata { span: e.span() },
//                         )
//                     },
//                 )
//                 .boxed();

//             let declaration = keyword!(Let)
//                 .ignore_then(ident)
//                 .then_ignore(symbol!(Equals))
//                 .then(expression.clone())
//                 .map_with(|(name, rhs), e| {
//                     AstNode::new(
//                         Expression::new_definition(name, rhs),
//                         ParsedMetadata { span: e.span() },
//                     )
//                 })
//                 .boxed();

//             let rtn = keyword!(Return)
//                 .ignore_then(expression.clone())
//                 .map_with(|expr, e| {
//                     AstNode::new(
//                         Expression::new_return(expr),
//                         ParsedMetadata { span: e.span() },
//                     )
//                 })
//                 .boxed();

//             let mtch_choice = recursive(|item| {
//                 choice((
//                     select! {
//                         Token::Number { value, unit, .. } => Expression::new_number(
//                             value, unit.map(|v| v.to_string() )),
//                         Token::Bool(val) => Expression::new_boolean(val) ,
//                         Token::Str(s) => Expression::new_string(s.to_string()) ,
//                     },
//                     ident
//                         .then_ignore(symbol!(Colon))
//                         .then(item.clone())
//                         .separated_by(symbol!(Comma))
//                         .allow_trailing()
//                         .collect::<BTreeMap<_, _>>()
//                         .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
//                         .map(Expression::new_object),
//                     item.clone()
//                         .separated_by(symbol!(Comma))
//                         .collect::<Vec<_>>()
//                         .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
//                         .map(Expression::new_array),
//                     keyword!(Let)
//                         .ignore_then(ident)
//                         .map(Expression::new_match_definition),
//                 ))
//                 .map_with(|expr, e| AstNode::new(expr, ParsedMetadata { span: e.span() }))
//             });

//             let mtch = keyword!(Match)
//                 .ignore_then(expression.clone())
//                 .then(
//                     mtch_choice
//                         .clone()
//                         .then_ignore(symbol!(Arrow))
//                         .then(expression.clone())
//                         .separated_by(symbol!(Comma))
//                         .collect::<Vec<(_, _)>>()
//                         .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly)),
//                 )
//                 .map_with(|(value, arms), e| {
//                     AstNode::new(
//                         Expression::<ParsedMetadata>::new_match(value, arms),
//                         ParsedMetadata { span: e.span() },
//                     )
//                 });

//             choice((rtn, declaration, sums, mtch))
//         })
// .separated_by(symbol!(Semicolon))
// .allow_trailing()
// .collect::<Vec<_>>()
// .recover_with(skip_then_retry_until(any().ignored(), end()))
//     })
//     .then_ignore(end())
// }
