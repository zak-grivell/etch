use crate::{Array, Ident, expression::*, result::Results};
use std::fmt::{Debug, Display};

pub trait Ast: Sized + Debug + Clone + PartialEq {
    type E: Clone + Debug + PartialEq;
    type T: Clone + Debug + PartialEq;

    type I: Clone + Debug + PartialEq + PartialEq + Eq + PartialOrd + Ord;
    type U: Clone + Debug + PartialEq;
    type B: Clone + Debug + PartialEq;
}

#[derive(Debug, PartialEq, Clone)]
pub struct AstNode<A: Ast> {
    pub expr: Box<Expression<A>>,
    pub meta: A,
}

impl<T: Ast> AstNode<T> {
    pub fn transform<U: Ast>(
        self,
        f: impl FnOnce(Expression<T>, T) -> (Expression<U>, U),
    ) -> AstNode<U> {
        let (expr, meta) = f(*self.expr, self.meta);

        AstNode {
            expr: Box::new(expr),
            meta,
        }
    }

    pub fn try_transform<U: Ast, E>(
        self,
        f: impl FnOnce(Expression<T>, T) -> Results<(Expression<U>, U), E>,
    ) -> Results<AstNode<U>, E> {
        f(*self.expr, self.meta).map(|(expr, meta)| AstNode {
            meta,
            expr: Box::new(expr),
        })
    }

    pub fn new(expr: Expression<T>, meta: T) -> Self {
        Self {
            meta,
            expr: Box::new(expr),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol {
    pub id: u32,
    pub original: String,
}

impl Symbol {
    pub fn dummy(name: String) -> Self {
        Symbol {
            id: 0,
            original: name,
        }
    }
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
        meta: Self::From,
    ) -> Results<(Definition<Self::To>, Self::To), Self::Error>;

    fn transform_return(
        &mut self,
        rtn: Return<Self::From>,
        meta: Self::From,
    ) -> Results<(Return<Self::To>, Self::To), Self::Error>;

    fn transform_match(
        &mut self,
        mtch: Match<Self::From>,
        meta: Self::From,
    ) -> Results<(Match<Self::To>, Self::To), Self::Error>;
    fn transform_ident(
        &mut self,
        ident: Ident<Self::From>,
        meta: Self::From,
    ) -> Results<(Ident<Self::To>, Self::To), Self::Error>;
    fn transform_array(
        &mut self,
        array: Array<Self::From>,
        meta: Self::From,
    ) -> Results<(Array<Self::To>, Self::To), Self::Error>;
    fn transform_object(
        &mut self,
        object: Object<Self::From>,
        meta: Self::From,
    ) -> Results<(Object<Self::To>, Self::To), Self::Error>;
    fn transform_lambda(
        &mut self,
        lambda: Lambda<Self::From>,
        meta: Self::From,
    ) -> Results<(Lambda<Self::To>, Self::To), Self::Error>;
    fn transform_number(
        &mut self,
        number: NumberLiteral,
        meta: Self::From,
    ) -> Results<(NumberLiteral, Self::To), Self::Error>;
    fn transform_boolean(
        &mut self,
        boolean: BooleanLiteral,
        meta: Self::From,
    ) -> Results<(BooleanLiteral, Self::To), Self::Error>;
    fn transform_string(
        &mut self,
        string: StringLiteral,
        meta: Self::From,
    ) -> Results<(StringLiteral, Self::To), Self::Error>;
    fn transform_unary_op(
        &mut self,
        unary_operation: UnaryOperation<Self::From>,
        meta: Self::From,
    ) -> Results<(UnaryOperation<Self::To>, Self::To), Self::Error>;

    fn transform_binary_op(
        &mut self,
        binary_operation: BinaryOperation<Self::From>,
        meta: Self::From,
    ) -> Results<(BinaryOperation<Self::To>, Self::To), Self::Error>;
    fn transform_call(
        &mut self,
        call: Call<Self::From>,
        meta: Self::From,
    ) -> Results<(Call<Self::To>, Self::To), Self::Error>;
    fn transform_object_access(
        &mut self,
        object_access: ObjectAccess<Self::From>,
        meta: Self::From,
    ) -> Results<(ObjectAccess<Self::To>, Self::To), Self::Error>;

    fn transform(
        &mut self,
        ast_node: AstNode<Self::From>,
    ) -> Results<AstNode<Self::To>, Self::Error> {
        ast_node.try_transform(|expression, meta| match expression {
            Expression::Definition(definition) => self
                .transform_definition(definition, meta)
                .map(|(expr, meta)| (Expression::Definition(expr), meta)),
            Expression::Return(rtn) => self
                .transform_return(rtn, meta)
                .map(|(expr, meta)| (Expression::Return(expr), meta)),
            Expression::Match(mtch) => self
                .transform_match(mtch, meta)
                .map(|(expr, meta)| (Expression::Match(expr), meta)),
            Expression::Ident(ident) => self
                .transform_ident(ident, meta)
                .map(|(expr, meta)| (Expression::Ident(expr), meta)),
            Expression::Number(number) => self
                .transform_number(number, meta)
                .map(|(expr, meta)| (Expression::Number(expr), meta)),
            Expression::String(string) => self
                .transform_string(string, meta)
                .map(|(expr, meta)| (Expression::String(expr), meta)),
            Expression::Boolean(boolean) => self
                .transform_boolean(boolean, meta)
                .map(|(expr, meta)| (Expression::Boolean(expr), meta)),
            Expression::Array(array) => self
                .transform_array(array, meta)
                .map(|(expr, meta)| (Expression::Array(expr), meta)),
            Expression::Object(object) => self
                .transform_object(object, meta)
                .map(|(expr, meta)| (Expression::Object(expr), meta)),
            Expression::Lambda(lambda) => self
                .transform_lambda(lambda, meta)
                .map(|(expr, meta)| (Expression::Lambda(expr), meta)),
            Expression::BinaryOperation(binary_operation) => self
                .transform_binary_op(binary_operation, meta)
                .map(|(expr, meta)| (Expression::BinaryOperation(expr), meta)),
            Expression::UnaryOperation(unary_operation) => self
                .transform_unary_op(unary_operation, meta)
                .map(|(expr, meta)| (Expression::UnaryOperation(expr), meta)),
            Expression::Call(call) => self
                .transform_call(call, meta)
                .map(|(expr, meta)| (Expression::Call(expr), meta)),
            Expression::ObjectAccess(object_access) => self
                .transform_object_access(object_access, meta)
                .map(|(expr, meta)| (Expression::ObjectAccess(expr), meta)),
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
