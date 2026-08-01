use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use derive_more::From;

use crate::Ast;

// use crate::Symbol;

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
    pub options: BTreeSet<A::Type>,
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
pub struct NumberType;
#[derive(Clone, Debug, PartialEq)]
pub struct BooleanType;
#[derive(Clone, Debug, PartialEq)]
pub struct NoneType;
#[derive(Clone, Debug, PartialEq)]
pub struct NeverType;

#[derive(Clone, Debug, PartialEq, From)]
pub enum Type<A: Ast> {
    Node(NodeType),
    Number(NumberType),
    String(StringType),
    Boolean(BooleanType),
    Object(ObjectType<A>),
    Array(ArrayType<A>),
    Lambda(LambdaType<A>),
    Union(UnionType<A>),
    Tuple(TupleType<A>),
    None(NoneType),
    Never(NeverType),
}

// impl<A: Ast> Type<A> {
//     pub fn map<U: Ord>(self, mut f: impl FnMut(T) -> U) -> Type<U> {
//         match self {
//             Type::Node => Type::Node,
//             Type::Number => Type::Number,
//             Type::String => Type::String,
//             Type::Boolean => Type::Boolean,
//             Type::Void => Type::Void,
//             Type::Object { fields } => Type::Object {
//                 fields: fields.into_iter().map(|(k, v)| (k, f(v))).collect(),
//             },
//             Type::Tuple { v } => Type::Tuple {
//                 v: v.into_iter().map(&mut f).collect(),
//             },
//             Type::Array(t) => Type::Array(Box::new(f(*t))),
//             Type::Optional(t) => Type::Optional(Box::new(f(*t))),
//             Type::Lambda { params, rtn } => Type::Lambda {
//                 params: params.into_iter().map(|(k, v)| (k, f(v))).collect(),
//                 rtn: Box::new(f(*rtn)),
//             },
//             Type::Union { options } => Type::Union {
//                 options: options.into_iter().map(f).collect(),
//             },
//             Type::Wrapper { name, inner } => Type::Wrapper {
//                 name,
//                 inner: Box::new(f(*inner)),
//             },
//             Type::Never => Type::Never,
//         }
//     }

//     pub fn try_map<U: Ord>(self, mut f: impl FnMut(T) -> Option<U>) -> Option<Type<U>> {
//         Some(match self {
//             Type::Tuple { v } => Type::Tuple {
//                 v: v.into_iter().map(&mut f).collect::<Option<_>>()?,
//             },
//             Type::Node => Type::Node,
//             Type::Number => Type::Number,
//             Type::String => Type::String,
//             Type::Boolean => Type::Boolean,
//             Type::Void => Type::Void,
//             Type::Object { fields } => Type::Object {
//                 fields: fields
//                     .into_iter()
//                     .map(|(k, v)| Some((k, f(v)?)))
//                     .collect::<Option<_>>()?,
//             },
//             Type::Array(t) => Type::Array(Box::new(f(*t)?)),
//             Type::Optional(t) => Type::Optional(Box::new(f(*t)?)),
//             Type::Lambda { params, rtn } => Type::Lambda {
//                 params: params
//                     .into_iter()
//                     .map(|(k, v)| Some((k, f(v)?)))
//                     .collect::<Option<_>>()?,
//                 rtn: Box::new(f(*rtn)?),
//             },
//             Type::Union { options } => Type::Union {
//                 options: options.into_iter().map(&mut f).collect::<Option<_>>()?,
//             },
//             Type::Wrapper { name, inner } => Type::Wrapper {
//                 name,
//                 inner: Box::new(f(*inner)?),
//             },
//             Type::Never => Type::Never,
//         })
//     }
// }
// use std::fmt::{self, Display};

// use crate::Ast;

// impl Display for StrongType {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         Display::fmt(&self.0, f)
//     }
// }

// impl Display for PartialType {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             PartialType::Unknown => write!(f, "?"),
//             PartialType::T(t) => Display::fmt(&t, f),
//         }
//     }
// }

// impl<T: Display> Display for Type<T> {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Type::Node => write!(f, "node"),
//             Type::Number => write!(f, "number"),
//             Type::String => write!(f, "string"),
//             Type::Boolean => write!(f, "boolean"),
//             Type::Void => write!(f, "void"),
//             Type::Never => write!(f, "never"),

//             Type::Array(inner) => {
//                 write!(f, "[{inner}]")
//             }

//             Type::Optional(inner) => {
//                 write!(f, "{inner}?")
//             }

//             Type::Object { fields } => {
//                 write!(f, "{{")?;

//                 for (i, (name, ty)) in fields.iter().enumerate() {
//                     if i != 0 {
//                         write!(f, ", ")?;
//                     }

//                     write!(f, "{name}: {ty}")?;
//                 }

//                 write!(f, "}}")
//             }

//             Type::Lambda { params, rtn } => {
//                 write!(f, "(")?;

//                 for (i, (name, ty)) in params.iter().enumerate() {
//                     if i != 0 {
//                         write!(f, ", ")?;
//                     }

//                     write!(f, "{name}: {ty}")?;
//                 }

//                 write!(f, ") -> {rtn}")
//             }

//             Type::Union { options } => {
//                 for (i, ty) in options.iter().enumerate() {
//                     if i != 0 {
//                         write!(f, " | ")?;
//                     }

//                     write!(f, "{ty}")?;
//                 }

//                 Ok(())
//             }

//             Type::Wrapper { name, inner } => {
//                 write!(f, "{name}({inner})")
//             }

//             Type::Tuple { v } => {
//                 write!(
//                     f,
//                     "({})",
//                     v.iter()
//                         .map(|v| v.to_string())
//                         .collect::<Vec<_>>()
//                         .join(", ")
//                 )
//             }
//         }
//     }
// }
