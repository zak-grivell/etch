use crate::{Array, Ident, Span, expression::*, result::Results};
use chumsky::span::SimpleSpan;
use std::fmt::{Debug, Display};

pub trait Ast: Sized {
    type E: Clone + Debug + PartialEq;
    type T: Clone + Debug + PartialEq;

    type I: Clone + Debug + PartialEq + PartialEq + Eq + PartialOrd + Ord;
    type U: Clone + Debug + PartialEq;
    type B: Clone + Debug + PartialEq;

    fn expression(self) -> Expression<Self>;
}

#[derive(Debug, PartialEq, Clone)]
pub struct AstNode<I: Debug + PartialEq + Clone, S: Debug + PartialEq + Clone> {
    pub meta: I,
    pub expr: Box<S>,
}

// impl<T: Ast> AstNode<T> {
//     pub fn transform<U: Ast>(self, f: impl FnOnce(T, Span) -> U) -> AstNode<U> {
//         AstNode {
//             meta: self.meta,
//             expr: Box::new(f(*self.expr, self.meta)),
//         }
//     }

//     pub fn try_transform<U: Ast, E>(
//         self,
//         f: impl FnOnce(T, Span) -> Results<U, E>,
//     ) -> Results<AstNode<U>, E> {
//         f(*self.expr, self.meta).map(|v| AstNode {
//             meta: self.meta,
//             expr: Box::new(v),
//         })
//     }

//     pub fn new(value: T, span: SimpleSpan) -> Self {
//         Self {
//             meta: span,
//             expr: Box::new(value),
//         }
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol {
    pub id: u32,
    pub original: String,
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.original)
    }
}

pub trait AstTransform {
    type Error;
    type From: Ast;
    type To: Ast;

    fn transform_definition(
        &mut self,
        definition: Definition<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_return(
        &mut self,
        rtn: Return<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_match(
        &mut self,
        mtch: Match<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_ident(
        &mut self,
        ident: Ident<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_array(
        &mut self,
        array: Array<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_object(
        &mut self,
        object: Object<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_lambda(
        &mut self,
        lambda: Lambda<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_number(
        &mut self,
        number: NumberLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_boolean(
        &mut self,
        boolean: BooleanLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_string(
        &mut self,
        string: StringLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_unary_op(
        &mut self,
        unary_operation: UnaryOperation<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;

    fn transform_binary_op(
        &mut self,
        binary_operation: BinaryOperation<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_call(
        &mut self,
        call: Call<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;
    fn transform_object_access(
        &mut self,
        object_access: ObjectAccess<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error>;

    fn transform(
        &mut self,
        ast_node: AstNode<Self::From>,
    ) -> Results<AstNode<Self::To>, Self::Error> {
        ast_node.try_transform(|inner, span| match inner.expression() {
            Expression::Definition(definition) => self.transform_definition(definition, span),
            Expression::Return(rtn) => self.transform_return(rtn, span),
            Expression::Match(mtch) => self.transform_match(mtch, span),
            Expression::Ident(ident) => self.transform_ident(ident, span),
            Expression::Number(number) => self.transform_number(number, span),
            Expression::String(string) => self.transform_string(string, span),
            Expression::Boolean(boolean) => self.transform_boolean(boolean, span),
            Expression::Array(array) => self.transform_array(array, span),
            Expression::Object(object) => self.transform_object(object, span),
            Expression::Lambda(lambda) => self.transform_lambda(lambda, span),
            Expression::BinaryOperation(binary_operation) => {
                self.transform_binary_op(binary_operation, span)
            }
            Expression::UnaryOperation(unary_operation) => {
                self.transform_unary_op(unary_operation, span)
            }
            Expression::Call(call) => self.transform_call(call, span),
            Expression::ObjectAccess(object_access) => {
                self.transform_object_access(object_access, span)
            }
        })
    }

    fn transform_all(
        &mut self,
        ast_nodes: Vec<AstNode<Self::From>>,
    ) -> Results<Vec<AstNode<Self::To>>, Self::Error> {
        ast_nodes
            .into_iter()
            .map(|node| self.transform(node))
            .collect()
    }
}
