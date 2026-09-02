use derive_more::From;

use crate::Type;
use crate::ast::Ast;
use crate::primative::Primative;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm<A: Ast> {
    pub pattern: Option<A::Pattern>,
    pub condition: Option<Box<A::Expression>>,
    pub result: Box<A::Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Match<A: Ast> {
    pub on: Box<A::Expression>,
    pub arms: Vec<MatchArm<A>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object<A: Ast> {
    pub fields: BTreeMap<String, A::Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lambda<A: Ast> {
    pub params: BTreeMap<A::Ident, Type<A>>,
    pub body: Box<A::Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call<A: Ast> {
    pub expression: Box<A::Expression>,
    pub args: BTreeMap<String, A::Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectAccess<A: Ast> {
    pub expression: Box<A::Expression>,
    pub field: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnaryOperation<A: Ast> {
    pub arg: Box<A::Expression>,
    pub op: A::U,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BinaryOperation<A: Ast> {
    pub lhs: Box<A::Expression>,
    pub rhs: Box<A::Expression>,
    pub op: A::B,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BinaryOperator {
    Equal,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
    GreaterThanOrEqual,
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
            BinaryOperator::Equal => write!(f, "=="),
            BinaryOperator::LessThan => write!(f, "<"),
            BinaryOperator::GreaterThan => write!(f, ">"),
            BinaryOperator::LessThanOrEqual => write!(f, "<="),
            BinaryOperator::GreaterThanOrEqual => write!(f, ">="),
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
    pub items: Vec<A::Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ident<A: Ast> {
    pub ident: A::Ident,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block<A: Ast> {
    pub body: Vec<A::Statement>,
}

#[derive(Debug, PartialEq, Clone, From)]
pub enum Expression<A: Ast> {
    Match(Match<A>),
    Ident(Ident<A>),
    Primative(Primative),
    Array(Array<A>),
    Object(Object<A>),
    Lambda(Lambda<A>),

    Node(Node),
    UnaryOperation(UnaryOperation<A>),
    BinaryOperation(BinaryOperation<A>),
    Call(Call<A>),
    ObjectAccess(ObjectAccess<A>),
    Block(Block<A>),
}
