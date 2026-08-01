use std::collections::BTreeMap;

use derive_more::From;

use crate::{Ast, Ident, primative::Primative};

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDestructure<A: Ast> {
    pub fields: BTreeMap<String, A::Pattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayDestructure<A: Ast> {
    pub values: Vec<A::Pattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDestructure {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, From)]
pub enum Pattern<A: Ast> {
    Object(ObjectDestructure<A>),
    Array(ArrayDestructure<A>),
    Enum(EnumDestructure),
    Binding(Ident<A>),
    Primative(Primative),
}
