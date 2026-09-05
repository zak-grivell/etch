use std::collections::BTreeMap;
use std::fmt::Debug;

use derive_more::From;

use crate::Ast;

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectType<A: Ast> {
    pub fields: BTreeMap<String, A::Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrayType<A: Ast> {
    pub item_type: Box<A::Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptionalType<A: Ast> {
    pub inner: Box<A::Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LambdaType<A: Ast> {
    pub params: BTreeMap<A::Ident, A::Type>,
    pub rtn: Box<A::Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnionType<A: Ast> {
    pub options: Vec<A::Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TupleType<A: Ast> {
    pub types: Vec<A::Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeType;
#[derive(Clone, Debug, PartialEq)]
pub struct StringType;
#[derive(Clone, Debug, PartialEq)]
pub struct NumberType {
    /// Display symbol for a concrete numeric unit (for example `V` or `Ω`).
    pub symbol: Option<String>,
}

impl NumberType {
    pub fn scalar() -> Self {
        Self { symbol: None }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BooleanType;
#[derive(Clone, Debug, PartialEq)]
pub struct NoneType;
#[derive(Clone, Debug, PartialEq)]
pub struct NeverType;

#[derive(Clone, Debug, PartialEq, From)]
pub enum Type<A: Ast> {
    Node(NodeType),
    Named(NamedType),
    Number(NumberType),
    String(StringType),
    Boolean(BooleanType),
    Object(ObjectType<A>),
    Array(ArrayType<A>),
    Optional(OptionalType<A>),
    Lambda(LambdaType<A>),
    Union(UnionType<A>),
    Tuple(TupleType<A>),
    None(NoneType),
    Never(NeverType),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedType {
    pub name: String,
}
