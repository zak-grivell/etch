use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Statement<'src> {
    Definition {
        name: &'src str,
        rhs: Box<Expression<'src>>,
    },
    Expression(Expression<'src>),
    Return {
        value: Box<Expression<'src>>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression<'src> {
    // Builtins
    Match {
        value: Box<Expression<'src>>,
        conds: Vec<(Expression<'src>, Expression<'src>)>,
    },
    Lambda {
        params: HashMap<&'src str, Option<&'src str>>,
        body: Vec<Statement<'src>>,
    },
    Block {
        body: Vec<Statement<'src>>,
    },

    Ident(&'src str),

    // Primatives
    Number {
        value: f64,
        unit: Option<&'src str>,
    },
    String(&'src str),
    Object(HashMap<&'src str, Expression<'src>>),
    Boolean(bool),
    Array(Vec<Expression<'src>>),
    Some(Box<Expression<'src>>),
    None,

    // Operations
    Call {
        expression: Box<Expression<'src>>,
        args: HashMap<&'src str, Expression<'src>>,
    },
    ObjectAcess {
        expr: Box<Expression<'src>>,
        field: &'src str,
    },

    Negate(Box<Expression<'src>>),
    Flip(Box<Expression<'src>>),

    Add(Box<Expression<'src>>, Box<Expression<'src>>),
    Sub(Box<Expression<'src>>, Box<Expression<'src>>),
    Mul(Box<Expression<'src>>, Box<Expression<'src>>),
    Div(Box<Expression<'src>>, Box<Expression<'src>>),

    Union(Box<Expression<'src>>, Box<Expression<'src>>),
    Wire(Box<Expression<'src>>, Box<Expression<'src>>),
}
