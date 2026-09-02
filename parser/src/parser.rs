use ast::{
    Ast, AstNode, BinaryOperator, Expression, Ident, Import, Pattern, Primative, Program, Span,
    Statement, Type, UnaryOperator,
};
use chumsky::extra::ParserExtra;
use chumsky::input::ValueInput;
use chumsky::prelude::*;
use std::collections::BTreeMap;

use crate::lexer::keyword;
use crate::lexer::{Keyword, Symbols, Token, symbol};

type Extra<'src> = extra::Err<Rich<'src, Token<'src>, Span>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParsedNode;

type Parsed<T> = AstNode<T, ParsedNode>;
type ParsedExpression = Parsed<Expression<ParsedNode>>;
type LambdaParts = (BTreeMap<String, Parsed<Type<ParsedNode>>>, ParsedExpression);

impl Ast for ParsedNode {
    type Node<T> = AstNode<T, Self>;

    type Expression = Self::Node<Expression<Self>>;
    type Pattern = Self::Node<Pattern<Self>>;
    type Type = Self::Node<Type<Self>>;
    type Statement = Self::Node<Statement<Self>>;

    type Ident = String;
    type B = BinaryOperator;
    type U = UnaryOperator;

    type Meta = Span;
}

pub trait SpannedExt<'src, I, O, E>: Parser<'src, I, O, E> + Sized
where
    I: Input<'src, Span = Span>,
    E: ParserExtra<'src, I>,
{
    fn spanned_node(self) -> impl Parser<'src, I, AstNode<O, ParsedNode>, E> {
        self.map_with(|inner, e| AstNode {
            inner,
            meta: e.span(),
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

fn collect_unique<'src, V>(
    entries: Vec<(String, V)>,
    span: Span,
) -> Result<BTreeMap<String, V>, Rich<'src, Token<'src>, Span>> {
    let mut fields = BTreeMap::new();

    for (name, value) in entries {
        if fields.insert(name.clone(), value).is_some() {
            return Err(Rich::custom(span, format!("duplicate field `{name}`")));
        }
    }

    Ok(fields)
}

pub fn parse<'src, I>() -> impl Parser<'src, I, Vec<Parsed<Program<ParsedNode>>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    program_parse().then_ignore(end())
}

pub fn program_parse<'src, I>()
-> impl Parser<'src, I, Vec<Parsed<Program<ParsedNode>>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let import = keyword!(From)
        .ignore_then(select! {
            Token::Str(path) => path.to_string()
        })
        .then_ignore(keyword!(Import))
        .then(pattern_parse())
        .map(|(path, imports)| Import { imports, path })
        .map(Program::Import)
        .spanned_node()
        .boxed();

    let export = keyword!(Export)
        .ignore_then(keyword!(Let))
        .ignore_then(pattern_parse())
        .then_ignore(symbol!(Equals))
        .then(expression_parse())
        .map(|(lhs, rhs)| ast::Definition { lhs, rhs })
        .map(Program::Export)
        .spanned_node()
        .boxed();

    let statement = statement_parse(expression_parse().boxed())
        .map(|statement: Parsed<Statement<ParsedNode>>| statement.map(Program::Statement))
        .boxed();

    choice((import, export, statement))
        .separated_by(symbol!(Semicolon))
        .allow_trailing()
        .collect::<Vec<_>>()
        .recover_with(skip_then_retry_until(
            any().ignored(),
            choice((symbol!(Semicolon).ignored(), end())),
        ))
}

fn statement_parse<'src, I, P>(
    expression_parse: P,
) -> impl Parser<'src, I, Parsed<Statement<ParsedNode>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
    P: Parser<'src, I, Parsed<Expression<ParsedNode>>, Extra<'src>> + Clone + 'src,
{
    let definition = keyword!(Let)
        .ignore_then(pattern_parse())
        .then_ignore(symbol!(Equals))
        .then(expression_parse.clone())
        .map(|(lhs, rhs)| ast::Definition { lhs, rhs })
        .map(Statement::Definition)
        .spanned_node()
        .boxed();

    let type_definition = keyword!(Type)
        .ignore_then(select! {
            Token::Identifier(ident) => ident.to_string()
        })
        .then_ignore(symbol!(Equals))
        .then(type_parse())
        .map(|(lhs, rhs)| ast::TypeDefinition { lhs, rhs })
        .map(Statement::TypeDefinition)
        .spanned_node()
        .boxed();

    let rtn = keyword!(Return)
        .ignore_then(expression_parse.clone())
        .map(|value| ast::Return { value })
        .map(Statement::Return)
        .spanned_node()
        .boxed();

    let expression = expression_parse
        .map(|expr: Parsed<Expression<ParsedNode>>| expr.map(Statement::Expression))
        .boxed();

    choice((definition, type_definition, rtn, expression))
}

pub fn statements_parse<'src, I>()
-> impl Parser<'src, I, Vec<Parsed<Statement<ParsedNode>>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    statement_parse(expression_parse().boxed())
        .separated_by(symbol!(Semicolon))
        .allow_trailing()
        .collect::<Vec<_>>()
        .recover_with(skip_then_retry_until(
            any().ignored(),
            choice((symbol!(Semicolon).ignored(), end())),
        ))
}

pub fn primative_parse<'src, I>() -> impl Parser<'src, I, Parsed<Primative>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    select! {
        Token::Number { value, .. } => Primative::Number(value),
        Token::Bool(val) => Primative::Boolean(val),
        Token::Str(s) => Primative::String(s.to_string()),
    }
    .spanned_node()
}

pub fn expression_parse<'src, I>()
-> impl Parser<'src, I, Parsed<Expression<ParsedNode>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let ident = select! {
        Token::Identifier(name) => name.to_string(),
    };

    recursive(
        |expression: Recursive<
            dyn Parser<'src, I, Parsed<Expression<ParsedNode>>, Extra<'src>>,
        >| {
            let primitive = primative_parse()
                .map(|node: Parsed<Primative>| node.map(Expression::Primative))
                .boxed();

            let node = symbol!(At)
                .to(Expression::Node(ast::Node))
                .spanned_node()
                .boxed();

            let object_field = ident
                .then(symbol!(Colon).ignore_then(expression.clone()).or_not())
                .map_with(|(name, value), e| {
                    let value = value.unwrap_or_else(|| AstNode {
                        inner: Expression::Ident(Ident {
                            ident: name.clone(),
                        }),
                        meta: e.span(),
                    });
                    (name, value)
                });

            let object = object_field
                .separated_by(symbol!(Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .try_map(collect_unique)
                .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly))
                .map(|fields| Expression::Object(ast::Object { fields }))
                .spanned_node()
                .boxed();

            let array = expression
                .clone()
                .separated_by(symbol!(Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
                .map(|items| Expression::Array(ast::Array { items }))
                .spanned_node()
                .boxed();

            let lambda = ident
                .then_ignore(symbol!(Colon))
                .then(type_parse())
                .separated_by(symbol!(Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .try_map(collect_unique)
                .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                .then_ignore(symbol!(Arrow))
                .then(expression.clone())
                .map(|(params, body): LambdaParts| {
                    let params = params
                        .into_iter()
                        .map(|(name, ty)| (name, ty.inner))
                        .collect::<BTreeMap<_, _>>();

                    Expression::Lambda(ast::Lambda {
                        params,
                        body: Box::new(body),
                    })
                })
                .spanned_node()
                .boxed();

            let block = statement_parse(expression.clone())
                .separated_by(symbol!(Semicolon))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly))
                .map(|body| Expression::Block(ast::Block { body }))
                .spanned_node()
                .boxed();

            let ident_expr = ident
                .map(|ident| Expression::Ident(Ident { ident }))
                .spanned_node()
                .boxed();

            let atom = choice((primitive, node, object, array, lambda, block, ident_expr)).boxed();

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
                    |expression: Parsed<Expression<ParsedNode>>, field: String, e| AstNode {
                        inner: Expression::ObjectAccess(ast::ObjectAccess {
                            expression: Box::new(expression),
                            field,
                        }),
                        meta: e.span(),
                    },
                )
                .boxed();

            let call = dot
                .foldl_with(
                    ident
                        .then_ignore(symbol!(Colon))
                        .then(expression.clone())
                        .separated_by(symbol!(Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .try_map(collect_unique)
                        .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                        .repeated(),
                    |expression: Parsed<Expression<ParsedNode>>,
                     args: BTreeMap<String, Parsed<Expression<ParsedNode>>>,
                     e| AstNode {
                        inner: Expression::Call(ast::Call {
                            expression: Box::new(expression),
                            args,
                        }),
                        meta: e.span(),
                    },
                )
                .boxed();

            let unary = choice((
                symbol!(Bang).to(UnaryOperator::Flip),
                symbol!(Minus).to(UnaryOperator::Negate),
            ))
            .repeated()
            .foldr_with(
                call,
                |op: UnaryOperator, arg: Parsed<Expression<ParsedNode>>, e| AstNode {
                    inner: Expression::UnaryOperation(ast::UnaryOperation {
                        arg: Box::new(arg),
                        op,
                    }),
                    meta: e.span(),
                },
            )
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
                    |lhs: Parsed<Expression<ParsedNode>>,
                     (op, rhs): (BinaryOperator, Parsed<Expression<ParsedNode>>),
                     e| AstNode {
                        inner: Expression::BinaryOperation(ast::BinaryOperation {
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                            op,
                        }),
                        meta: e.span(),
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
                    |lhs: Parsed<Expression<ParsedNode>>,
                     (op, rhs): (BinaryOperator, Parsed<Expression<ParsedNode>>),
                     e| AstNode {
                        inner: Expression::BinaryOperation(ast::BinaryOperation {
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                            op,
                        }),
                        meta: e.span(),
                    },
                )
                .boxed();

            let comparisons = sums
                .clone()
                .foldl_with(
                    choice((
                        symbol!(DoubleEquals).to(BinaryOperator::Equal),
                        symbol!(Le).to(BinaryOperator::LessThanOrEqual),
                        symbol!(Ge).to(BinaryOperator::GreaterThanOrEqual),
                        symbol!(Lt).to(BinaryOperator::LessThan),
                        symbol!(Gt).to(BinaryOperator::GreaterThan),
                    ))
                    .then(sums)
                    .repeated(),
                    |lhs: Parsed<Expression<ParsedNode>>,
                     (op, rhs): (BinaryOperator, Parsed<Expression<ParsedNode>>),
                     e| AstNode {
                        inner: Expression::BinaryOperation(ast::BinaryOperation {
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                            op,
                        }),
                        meta: e.span(),
                    },
                )
                .boxed();

            let unions = comparisons
                .clone()
                .foldl_with(
                    choice((
                        symbol!(Pipe).to(BinaryOperator::Union),
                        symbol!(BackArrow).to(BinaryOperator::Wire),
                    ))
                    .then(comparisons)
                    .repeated(),
                    |lhs: Parsed<Expression<ParsedNode>>,
                     (op, rhs): (BinaryOperator, Parsed<Expression<ParsedNode>>),
                     e| AstNode {
                        inner: Expression::BinaryOperation(ast::BinaryOperation {
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                            op,
                        }),
                        meta: e.span(),
                    },
                )
                .boxed();

            let match_arm = choice((
                pattern_parse()
                    .then(keyword!(If).ignore_then(expression.clone()).or_not())
                    .map(|(pattern, condition)| (Some(pattern), condition)),
                keyword!(If)
                    .ignore_then(expression.clone())
                    .map(|condition| (None, Some(condition))),
            ))
            .then_ignore(symbol!(Arrow))
            .then(expression.clone())
            .map(|((pattern, condition), result)| ast::MatchArm {
                pattern,
                condition: condition.map(Box::new),
                result: Box::new(result),
            });

            let mtch = keyword!(Match)
                .ignore_then(expression.clone())
                .then(
                    match_arm
                        .separated_by(symbol!(Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly)),
                )
                .map(|(on, arms)| {
                    Expression::Match(ast::Match {
                        on: Box::new(on),
                        arms,
                    })
                })
                .spanned_node()
                .boxed();

            choice((mtch, unions))
        },
    )
}

pub fn pattern_parse<'src, I>() -> impl Parser<'src, I, Parsed<Pattern<ParsedNode>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    let ident = select! {
        Token::Identifier(name) => name.to_string(),
    };

    recursive(|pattern| {
        let primitive = primative_parse()
            .map(|node: Parsed<Primative>| node.map(Pattern::Primative))
            .boxed();

        let binding = choice((keyword!(Let).ignore_then(ident), ident))
            .map(|ident| Pattern::Binding(Ident { ident }))
            .spanned_node()
            .boxed();

        let object_field = ident
            .then(symbol!(Colon).ignore_then(pattern.clone()).or_not())
            .map_with(|(name, value), e| {
                let value = value.unwrap_or_else(|| AstNode {
                    inner: Pattern::Binding(Ident {
                        ident: name.clone(),
                    }),
                    meta: e.span(),
                });
                (name, value)
            });

        let object = object_field
            .separated_by(symbol!(Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .try_map(collect_unique)
            .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly))
            .map(|fields| Pattern::Object(ast::ObjectDestructure { fields }))
            .spanned_node()
            .boxed();

        let array = pattern
            .clone()
            .separated_by(symbol!(Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
            .map(|values| Pattern::Array(ast::ArrayDestructure { values }))
            .spanned_node()
            .boxed();

        choice((primitive, object, array, binding))
    })
}

pub fn type_parse<'src, I>() -> impl Parser<'src, I, Parsed<Type<ParsedNode>>, Extra<'src>>
where
    I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
    recursive(
        |ty: Recursive<dyn Parser<'src, I, Parsed<Type<ParsedNode>>, Extra<'src>>>| {
            let base = choice((
                just(Token::Identifier("Node")).to(Type::Node(ast::NodeType)),
                just(Token::Identifier("Number")).to(Type::Number(ast::NumberType)),
                just(Token::Identifier("String")).to(Type::String(ast::StringType)),
                just(Token::Identifier("Bool")).to(Type::Boolean(ast::BooleanType)),
                just(Token::Identifier("None")).to(Type::None(ast::NoneType)),
                just(Token::Identifier("Never")).to(Type::Never(ast::NeverType)),
            ))
            .spanned_node()
            .boxed();

            let object = select! {
                Token::Identifier(name) => name.to_string()
            }
            .then_ignore(symbol!(Colon))
            .then(ty.clone())
            .separated_by(symbol!(Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .try_map(collect_unique)
            .delimited_by(symbol!(OpenCurly), symbol!(ClosedCurly))
            .map(|fields: BTreeMap<String, Parsed<Type<ParsedNode>>>| {
                Type::Object(ast::ObjectType { fields })
            })
            .spanned_node()
            .boxed();

            let array = ty
                .clone()
                .delimited_by(symbol!(OpenSquare), symbol!(ClosedSquare))
                .map(|item_type: Parsed<Type<ParsedNode>>| {
                    Type::Array(ast::ArrayType {
                        item_type: Box::new(item_type),
                    })
                })
                .spanned_node()
                .boxed();

            let tuple = ty
                .clone()
                .separated_by(symbol!(Comma))
                .at_least(2)
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
                .map(|types: Vec<Parsed<Type<ParsedNode>>>| Type::Tuple(ast::TupleType { types }))
                .spanned_node()
                .boxed();

            let lambda = select! {
                Token::Identifier(name) => name.to_string()
            }
            .then_ignore(symbol!(Colon))
            .then(ty.clone())
            .separated_by(symbol!(Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .try_map(collect_unique)
            .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis))
            .then_ignore(symbol!(Arrow))
            .then(ty.clone())
            .map(|(params, rtn)| {
                Type::Lambda(ast::LambdaType {
                    params,
                    rtn: Box::new(rtn),
                })
            })
            .spanned_node()
            .boxed();

            let atom = choice((
                object,
                array,
                lambda,
                tuple,
                base,
                ty.clone()
                    .delimited_by(symbol!(OpenParenthesis), symbol!(ClosedParenthesis)),
            ))
            .boxed();

            let optional = atom
                .foldl_with(
                    symbol!(Question).repeated(),
                    |inner: Parsed<Type<ParsedNode>>, _, e| AstNode {
                        inner: Type::Optional(ast::OptionalType {
                            inner: Box::new(inner),
                        }),
                        meta: e.span(),
                    },
                )
                .boxed();

            optional.clone().foldl_with(
                symbol!(Pipe).ignore_then(optional).repeated(),
                |lhs: Parsed<Type<ParsedNode>>, rhs, e| {
                    let options = match lhs.inner {
                        Type::Union(union) => union.options,
                        inner => vec![AstNode {
                            inner,
                            meta: lhs.meta,
                        }],
                    };

                    AstNode {
                        inner: Type::Union(ast::UnionType {
                            options: options.into_iter().chain(std::iter::once(rhs)).collect(),
                        }),
                        meta: e.span(),
                    }
                },
            )
        },
    )
}
