use core::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,

    Equals,
    Lt,
    Gt,
    Ge,
    Le,

    Override,
}

impl From<&str> for BinaryOperator {
    fn from(value: &str) -> Self {
        match value {
            "==" => Self::Equals,
            "<" => Self::Lt,
            ">" => Self::Gt,
            "<=" => Self::Le,
            ">=" => Self::Ge,
            "+" => Self::Add,
            "-" => Self::Subtract,
            "/" => Self::Divide,
            "*" => Self::Multiply,
            "<-" => Self::Override,
            _ => panic!("Should be binary operator: {}", value),
        }
    }
}

impl fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self {
            Self::Equals => "==",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::Le => "<=",
            Self::Ge => ">=",
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Divide => "/",
            Self::Multiply => "*",
            Self::Override => "<-",
        };

        write!(f, "{op}")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnaryOperator {
    Invert,
    Negate,
}

impl From<&str> for UnaryOperator {
    fn from(value: &str) -> Self {
        match value {
            "!" => Self::Invert,
            "-" => Self::Negate,
            _ => panic!("Should be ! or - got: {}", value),
        }
    }
}

impl fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self {
            Self::Invert => "!",
            Self::Negate => "-",
        };

        write!(f, "{op}")
    }
}
