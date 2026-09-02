use derive_more::From;

use crate::{Ast, Definition, Statement};

#[derive(Debug, PartialEq, Clone)]
pub struct Import<A: Ast> {
    pub imports: A::Pattern,
    pub path: String,
}

#[derive(Debug, PartialEq, Clone, From)]
pub enum Program<A: Ast> {
    Import(Import<A>),
    Export(Definition<A>),
    Statement(Statement<A>),
}
