use crate::ast::Ast;
use derive_more::From;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};

#[derive(Clone, Debug, PartialEq)]
pub struct Definition<A: Ast> {
    pub name: A::I,
    pub rhs: A::E,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Return<A: Ast> {
    pub expression: A::E,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Match<A: Ast> {
    pub value: A::E,
    pub conds: Vec<(A::E, A::E)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumberLiteral {
    pub value: f64,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StringLiteral {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object<A: Ast> {
    pub fields: BTreeMap<String, A::E>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lambda<A: Ast> {
    pub params: BTreeMap<A::I, A::T>,
    pub body: Vec<A::E>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call<A: Ast> {
    pub expression: A::E,
    pub args: BTreeMap<String, A::E>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectAccess<A: Ast> {
    pub expression: A::E,
    pub field: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnaryOperation<A: Ast> {
    pub arg: A::E,
    pub op: A::U,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BinaryOperation<A: Ast> {
    pub lhs: A::E,
    pub rhs: A::E,
    pub op: A::B,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BooleanLiteral {
    pub value: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ident<A: Ast> {
    pub value: A::I,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Union,
    Wire,
}

impl Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOperator::Add => write!(f, "+"),
            BinaryOperator::Sub => write!(f, "-"),
            BinaryOperator::Mul => write!(f, "*"),
            BinaryOperator::Div => write!(f, "/"),
            BinaryOperator::Union => write!(f, "|"),
            BinaryOperator::Wire => write!(f, "<-"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnaryOperator {
    Negate,
    Flip,
}

impl Display for UnaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOperator::Negate => write!(f, "-"),
            UnaryOperator::Flip => write!(f, "!"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Array<A: Ast> {
    pub items: Vec<A::E>,
}

#[derive(Debug, PartialEq, Clone, From)]
pub enum Expression<A: Ast + PartialEq + Clone + Debug> {
    Definition(Definition<A>),

    Return(Return<A>),
    Match(Match<A>),

    Ident(Ident<A>),
    Number(NumberLiteral),
    String(StringLiteral),
    Boolean(BooleanLiteral),
    Array(Array<A>),
    Object(Object<A>),
    Lambda(Lambda<A>),

    UnaryOperation(UnaryOperation<A>),
    BinaryOperation(BinaryOperation<A>),

    Call(Call<A>),
    ObjectAccess(ObjectAccess<A>),
}

impl<A: Ast> Expression<A> {
    pub fn new_definition(name: A::I, rhs: A::E) -> Self {
        Self::Definition(Definition { name, rhs })
    }

    pub fn new_return(expression: A::E) -> Self {
        Self::Return(Return { expression })
    }

    pub fn new_match(value: A::E, conds: Vec<(A::E, A::E)>) -> Self {
        Self::Match(Match { value, conds })
    }

    pub fn new_ident(ident: A::I) -> Self {
        Self::Ident(Ident { value: ident })
    }

    pub fn new_number(value: f64, unit: Option<String>) -> Self {
        Self::Number(NumberLiteral { value, unit })
    }

    pub fn new_string(value: String) -> Self {
        Self::String(StringLiteral { value })
    }

    pub fn new_boolean(value: bool) -> Self {
        Self::Boolean(BooleanLiteral { value })
    }

    pub fn new_array(elements: Vec<A::E>) -> Self {
        Self::Array(Array { items: elements })
    }

    pub fn new_object(fields: BTreeMap<String, A::E>) -> Self {
        Self::Object(Object { fields })
    }

    pub fn new_lambda(params: BTreeMap<A::I, A::T>, body: Vec<A::E>) -> Self {
        Self::Lambda(Lambda { params, body })
    }

    pub fn new_unary_operation(arg: A::E, op: A::U) -> Self {
        Self::UnaryOperation(UnaryOperation { arg, op })
    }

    pub fn new_binary_operation(lhs: A::E, rhs: A::E, op: A::B) -> Self {
        Self::BinaryOperation(BinaryOperation { lhs, rhs, op })
    }

    pub fn new_call(expression: A::E, args: BTreeMap<String, A::E>) -> Self {
        Self::Call(Call { expression, args })
    }

    pub fn new_object_access(expression: A::E, field: String) -> Self {
        Self::ObjectAccess(ObjectAccess { expression, field })
    }
}
