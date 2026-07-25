use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use crate::Symbol;

#[derive(Clone, Debug, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub struct StrongType(pub Type<StrongType>);

#[derive(Clone, Debug, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub enum PartialType {
    T(Type<PartialType>),
    Unknown,
}

impl PartialType {
    fn unwrap(self) -> StrongType {
        match self {
            PartialType::T(t) => StrongType(t.map(|v| v.unwrap())),
            PartialType::Unknown => panic!("should be typed"),
        }
    }
}

impl From<StrongType> for PartialType {
    fn from(value: StrongType) -> Self {
        PartialType::T(value.0.map(PartialType::from))
    }
}

#[derive(Clone, Debug, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub enum Type<T> {
    Number,
    String,
    Boolean,
    Object {
        fields: BTreeMap<String, T>,
    },
    Array(Box<T>),
    Optional(Box<T>),
    Lambda {
        params: BTreeMap<Symbol, T>,
        rtn: Box<T>,
    },
    Union {
        options: BTreeSet<T>,
    },
    Void,
    Wrapper {
        name: String,
        inner: Box<T>,
    },
    Never,
}

impl<T> Type<T> {
    fn map<U: Ord>(self, mut f: impl FnMut(T) -> U) -> Type<U> {
        match self {
            Type::Number => Type::Number,
            Type::String => Type::String,
            Type::Boolean => Type::Boolean,
            Type::Void => Type::Void,
            Type::Object { fields } => Type::Object {
                fields: fields.into_iter().map(|(k, v)| (k, f(v))).collect(),
            },
            Type::Array(t) => Type::Array(Box::new(f(*t))),
            Type::Optional(t) => Type::Optional(Box::new(f(*t))),
            Type::Lambda { params, rtn } => Type::Lambda {
                params: params.into_iter().map(|(k, v)| (k, f(v))).collect(),
                rtn: Box::new(f(*rtn)),
            },
            Type::Union { options } => Type::Union {
                options: options.into_iter().map(f).collect(),
            },
            Type::Wrapper { name, inner } => Type::Wrapper {
                name,
                inner: Box::new(f(*inner)),
            },
            Type::Never => Type::Never,
        }
    }
}
use std::fmt::{self, Display};

impl Display for StrongType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Display for PartialType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartialType::Unknown => write!(f, "?"),
            PartialType::T(t) => Display::fmt(&t, f),
        }
    }
}

impl<T: Display> Display for Type<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Boolean => write!(f, "boolean"),
            Type::Void => write!(f, "void"),
            Type::Never => write!(f, "never"),

            Type::Array(inner) => {
                write!(f, "[{inner}]")
            }

            Type::Optional(inner) => {
                write!(f, "{inner}?")
            }

            Type::Object { fields } => {
                write!(f, "{{")?;

                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }

                    write!(f, "{name}: {ty}")?;
                }

                write!(f, "}}")
            }

            Type::Lambda { params, rtn } => {
                write!(f, "(")?;

                for (i, (name, ty)) in params.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }

                    write!(f, "{name}: {ty}")?;
                }

                write!(f, ") -> {rtn}")
            }

            Type::Union { options } => {
                for (i, ty) in options.iter().enumerate() {
                    if i != 0 {
                        write!(f, " | ")?;
                    }

                    write!(f, "{ty}")?;
                }

                Ok(())
            }

            Type::Wrapper { name, inner } => {
                write!(f, "{name}({inner})")
            }
        }
    }
}
