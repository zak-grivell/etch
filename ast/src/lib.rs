mod ast;
mod expression;
mod pattern;
mod primative;
mod program;
mod result;
mod statement;
mod types;

use chumsky::span::SimpleSpan;

pub type Span = SimpleSpan;

pub use ast::*;
pub use ast_macros::transformer;
pub use expression::*;
pub use pattern::*;
pub use primative::*;
pub use program::*;
pub use result::*;
pub use statement::*;
pub use types::*;
