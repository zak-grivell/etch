use derive_more::From;

use crate::{Ast, Statement};

#[derive(Debug, PartialEq, Clone)]
pub struct Import<A: Ast> {
    pub imports: A::Pattern,
    pub path: String,
}

#[derive(Debug, PartialEq, Clone, From)]
pub enum Program<A: Ast> {
    Import(Import<A>),
    Statement(Statement<A>),
}
