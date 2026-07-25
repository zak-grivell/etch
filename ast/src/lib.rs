mod ast;
mod expression;
mod operator;
mod result;
mod types;

use chumsky::span::SimpleSpan;

pub type Span = SimpleSpan;

pub use ast::*;
pub use expression::*;
pub use result::*;
pub use types::*;
