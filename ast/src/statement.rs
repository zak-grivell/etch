use derive_more::From;

use crate::{Ast, Expression};

#[derive(Clone, Debug, PartialEq)]
pub struct Definition<A: Ast> {
    pub lhs: A::Pattern,
    pub rhs: A::Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDefinition<A: Ast> {
    pub lhs: A::Ident,
    pub rhs: A::Type,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Return<A: Ast> {
    pub value: A::Expression,
}

#[derive(Debug, PartialEq, Clone, From)]
pub enum Statement<A: Ast> {
    Definition(Definition<A>),
    TypeDefinition(TypeDefinition<A>),
    Return(Return<A>),
    Expression(Expression<A>),
}
