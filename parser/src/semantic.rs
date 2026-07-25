use crate::resolver::SymbolicExpression;
use ast::{
    Ast, AstNode, AstTransform, BinaryOperation, BinaryOperator, Call, Definition, Expression,
    Lambda, Match, Object, ObjectAccess, PartialType, Results, Return, Span, Symbol, Type,
    UnaryOperation, UnaryOperator,
};
use chumsky::span::{SpanWrap, Spanned};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct PartiallyTypedExpression {
    t: PartialType,
    expression: Expression<PartiallyTypedExpression>,
}

impl Ast for PartiallyTypedExpression {
    type E = AstNode<Self>;
    type T = PartialType;

    type I = Option<Symbol>;

    type U = Option<TypedUnaryOperator>;

    type B = Option<TypedBinaryOperator>;

    fn expression(self) -> Expression<Self> {
        self.expression
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedUnaryOperator {
    FlipBool,
    NegateNumber,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedBinaryOperator {
    AddNumbers,
    SubNumbers,
    DivNumbers,
    MultNumbers,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemanticError {
    TypeMismatch {
        expected: PartialType,
        found: PartialType,
    },

    InvalidBinaryOperation {
        operator: BinaryOperator,
        left: PartialType,
        right: PartialType,
    },

    InvalidUrinaryOperation {
        operator: UnaryOperator,
        t: PartialType,
    },

    InvalidFieldAccess {
        object: PartialType,
        field: String,
    },

    ArgumentMissing {
        name: String,
        t: PartialType,
    },

    InvalidArgumentType {
        name: String,
        got: PartialType,
        expected: PartialType,
    },

    ExtraArgument {
        name: String,
    },
}
use std::fmt::{self, Display};

impl Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemanticError::TypeMismatch { expected, found } => {
                write!(f, "expected type `{expected}`, but found `{found}`")
            }

            SemanticError::InvalidBinaryOperation {
                operator,
                left,
                right,
            } => {
                write!(
                    f,
                    "cannot apply binary operator `{operator}` to values of type `{left}` and `{right}`"
                )
            }

            SemanticError::InvalidUrinaryOperation { operator, t } => {
                write!(
                    f,
                    "cannot apply unary operator `{operator}` to value of type `{t}`"
                )
            }

            SemanticError::InvalidFieldAccess { object, field } => {
                write!(f, "type `{object}` has no field `{field}`")
            }

            SemanticError::ArgumentMissing { name, t } => {
                write!(f, "missing required argument `{name}` of type `{t}`")
            }

            SemanticError::InvalidArgumentType {
                name,
                expected,
                got,
            } => {
                write!(
                    f,
                    "argument `{name}` has type `{got}`, but `{expected}` was expected"
                )
            }

            SemanticError::ExtraArgument { name } => {
                write!(f, "unexpected argument `{name}`")
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct TypeResolver {
    types: BTreeMap<Symbol, PartialType>,
}

impl TypeResolver {
    pub fn new() -> TypeResolver {
        TypeResolver {
            types: BTreeMap::new(),
        }
    }
}

fn resolve_multiple_types(items: impl Iterator<Item = PartialType>) -> PartialType {
    let options = items.collect::<BTreeSet<_>>();

    match options.len() {
        0 => PartialType::T(Type::Void),
        1 => options.iter().next().unwrap().clone(),
        _ => PartialType::T(Type::Union { options }),
    }
}

// cannot return partially typed

impl AstTransform for TypeResolver {
    type Error = Spanned<SemanticError>;
    type From = SymbolicExpression;
    type To = PartiallyTypedExpression;

    fn transform_definition(
        &mut self,
        Definition { name, rhs }: Definition<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(rhs).map(|rhs| {
            self.types
                .insert(name.clone().unwrap(), rhs.expr.t.clone());

            PartiallyTypedExpression {
                t: rhs.expr.t.clone(),
                expression: Expression::new_definition(name, rhs),
            }
        })
    }

    fn transform_return(
        &mut self,
        rtn: Return<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(rtn.expression)
            .map(|expression| PartiallyTypedExpression {
                t: PartialType::T(Type::Never),
                expression: Expression::new_return(expression),
            })
    }

    fn transform_match(
        &mut self,
        mtch: Match<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        let conds = mtch
            .conds
            .into_iter()
            .map(|(a, b)| self.transform(a).zip(self.transform(b)))
            .collect::<Results<Vec<_>, _>>();

        self.transform(mtch.value)
            .zip(conds)
            .map(|(value, conds)| PartiallyTypedExpression {
                t: PartialType::T(Type::Union {
                    options: conds.iter().map(|(_, v)| v.expr.t.clone()).collect(),
                }),
                expression: Expression::new_match(value, conds),
            })
    }

    fn transform_ident(
        &mut self,
        ident: ast::Ident<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(PartiallyTypedExpression {
            t: if let Some(ident) = &ident.value {
                self.types.get(ident).unwrap().clone()
            } else {
                PartialType::Unknown
            },
            expression: Expression::new_ident(ident.value),
        })
    }

    fn transform_number(
        &mut self,
        number: ast::NumberLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(PartiallyTypedExpression {
            t: PartialType::T(Type::Number),
            expression: Expression::new_number(number.value, number.unit),
        })
    }

    fn transform_string(
        &mut self,
        string: ast::StringLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(PartiallyTypedExpression {
            t: PartialType::T(Type::String),
            expression: Expression::new_string(string.value),
        })
    }

    fn transform_boolean(
        &mut self,
        boolean: ast::BooleanLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(PartiallyTypedExpression {
            t: PartialType::T(Type::Boolean),
            expression: Expression::new_boolean(boolean.value),
        })
    }

    fn transform_array(
        &mut self,
        array: ast::Array<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        let items = array
            .items
            .into_iter()
            .map(|e| self.transform(e))
            .collect::<Results<Vec<_>, _>>();

        items.map(|items| {
            let elem = items
                .first()
                .map(|e| e.expr.t.clone())
                .unwrap_or(PartialType::Unknown);

            PartiallyTypedExpression {
                t: PartialType::T(Type::Array(Box::new(elem))),
                expression: Expression::new_array(items),
            }
        })
    }

    fn transform_object(
        &mut self,
        Object { fields }: Object<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        let fields = fields
            .into_iter()
            .map(|(k, v)| self.transform(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>();

        fields.map(|fields| {
            let t = PartialType::T(Type::Object {
                fields: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.expr.t.clone()))
                    .collect(),
            });
            PartiallyTypedExpression {
                t,
                expression: Expression::new_object(fields),
            }
        })
    }

    fn transform_lambda(
        &mut self,
        lambda: Lambda<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.types.extend(
            lambda
                .params
                .iter()
                .map(|(s, t)| (s.as_ref().unwrap().clone(), t.clone())),
        );

        let body = lambda
            .body
            .into_iter()
            .map(|e| self.transform(e))
            .collect::<Results<Vec<_>, _>>();

        body.map(|body| {
            let return_type =
                resolve_multiple_types(body.iter().filter_map(
                    |expr| match &expr.expr.expression {
                        Expression::Return(Return { expression }) => {
                            Some(expression.expr.t.clone())
                        }
                        _ => None,
                    },
                ));

            let t = PartialType::T(Type::Lambda {
                params: lambda
                    .params
                    .iter()
                    .map(|(k, v)| (k.clone().unwrap(), v.clone()))
                    .collect(),
                rtn: Box::new(return_type),
            });

            PartiallyTypedExpression {
                t,
                expression: Expression::new_lambda(lambda.params, body),
            }
        })
    }

    fn transform_unary_op(
        &mut self,
        UnaryOperation { arg, op }: UnaryOperation<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(arg).flat_map(|arg| {
            let PartialType::T(ty) = arg.expr.t.clone() else {
                return Results::ok(PartiallyTypedExpression {
                    t: PartialType::Unknown,
                    expression: Expression::new_unary_operation(arg, None),
                });
            };

            match (&ty, &op) {
                (Type::Number, UnaryOperator::Negate) => Results::ok(PartiallyTypedExpression {
                    t: PartialType::T(Type::Number),
                    expression: Expression::new_unary_operation(
                        arg.clone(),
                        Some(TypedUnaryOperator::NegateNumber),
                    ),
                }),
                (Type::Boolean, UnaryOperator::Flip) => Results::ok(PartiallyTypedExpression {
                    t: PartialType::T(Type::Boolean),
                    expression: Expression::new_unary_operation(
                        arg,
                        Some(TypedUnaryOperator::FlipBool),
                    ),
                }),
                (_, _) => Results::with_error(
                    PartiallyTypedExpression {
                        t: PartialType::Unknown,
                        expression: Expression::new_unary_operation(arg, None),
                    },
                    SemanticError::InvalidUrinaryOperation {
                        operator: op.clone(),
                        t: PartialType::T(ty.clone()),
                    }
                    .with_span(span),
                ),
            }
        })
    }

    fn transform_binary_op(
        &mut self,
        BinaryOperation { lhs, rhs, op }: BinaryOperation<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(lhs)
            .zip(self.transform(rhs))
            .flat_map(|(lhs, rhs)| {
                let PartialType::T(lhs_type) = lhs.expr.t.clone() else {
                    return Results::ok(PartiallyTypedExpression {
                        t: PartialType::Unknown,
                        expression: Expression::new_binary_operation(lhs, rhs, None),
                    });
                };

                let PartialType::T(rhs_type) = rhs.expr.t.clone() else {
                    return Results::ok(PartiallyTypedExpression {
                        t: PartialType::Unknown,
                        expression: Expression::new_binary_operation(lhs, rhs, None),
                    });
                };

                let Some((result_type, typed_op)) = (match (&op, &lhs_type, &rhs_type) {
                    (BinaryOperator::Add, Type::Number, Type::Number) => Some((
                        PartialType::T(Type::Number),
                        TypedBinaryOperator::AddNumbers,
                    )),
                    (BinaryOperator::Sub, Type::Number, Type::Number) => Some((
                        PartialType::T(Type::Number),
                        TypedBinaryOperator::SubNumbers,
                    )),
                    (BinaryOperator::Mul, Type::Number, Type::Number) => Some((
                        PartialType::T(Type::Number),
                        TypedBinaryOperator::MultNumbers,
                    )),
                    (BinaryOperator::Div, Type::Number, Type::Number) => Some((
                        PartialType::T(Type::Number),
                        TypedBinaryOperator::DivNumbers,
                    )),
                    _ => None,
                }) else {
                    return Results::with_error(
                        PartiallyTypedExpression {
                            t: PartialType::Unknown,
                            expression: Expression::new_binary_operation(lhs, rhs, None),
                        },
                        SemanticError::InvalidBinaryOperation {
                            operator: op.clone(),
                            left: PartialType::T(lhs_type.clone()),
                            right: PartialType::T(rhs_type.clone()),
                        }
                        .with_span(span),
                    );
                };

                Results::ok(PartiallyTypedExpression {
                    t: result_type,
                    expression: Expression::new_binary_operation(lhs, rhs, Some(typed_op)),
                })
            })
    }

    fn transform_call(
        &mut self,
        Call { expression, args }: Call<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        let args = args
            .into_iter()
            .map(|(k, v)| self.transform(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>();

        args.zip(self.transform(expression))
            .flat_map(|(args, expression)| {
                let PartialType::T(ty) = expression.expr.t.clone() else {
                    return Results::ok(PartiallyTypedExpression {
                        t: PartialType::Unknown,
                        expression: Expression::new_call(expression, args),
                    });
                };

                let Type::Lambda { params, rtn } = ty.clone() else {
                    return Results::with_error(
                        PartiallyTypedExpression {
                            t: PartialType::Unknown,
                            expression: Expression::new_call(expression, args.clone()),
                        },
                        SemanticError::TypeMismatch {
                            expected: PartialType::T(Type::Lambda {
                                params: args
                                    .iter()
                                    .map(|(k, v)| (k.clone().unwrap(), v.expr.t.clone()))
                                    .collect(),
                                rtn: Box::new(PartialType::Unknown),
                            }),
                            found: PartialType::T(ty),
                        }
                        .with_span(span),
                    );
                };

                let result = PartiallyTypedExpression {
                    t: (*rtn).clone(),
                    expression: Expression::new_call(expression, args.clone()),
                };

                let missing = params
                    .iter()
                    .filter(|&(name, _)| !args.contains_key(&Some(name.clone())))
                    .map(|(name, expected)| {
                        SemanticError::ArgumentMissing {
                            name: name.original.clone(),
                            t: expected.clone(),
                        }
                        .with_span(span)
                    });

                let supplied = args.iter().filter_map(|(name, value)| {
                    let name = name.clone().unwrap();

                    match params.get(&name) {
                        Some(expected) if *expected == value.expr.t => None,
                        Some(expected) => Some(
                            SemanticError::InvalidArgumentType {
                                name: name.original.clone(),
                                expected: expected.clone(),
                                got: value.expr.t.clone(),
                            }
                            .with_span(span),
                        ),
                        None => Some(
                            SemanticError::ExtraArgument {
                                name: name.original.clone(),
                            }
                            .with_span(span),
                        ),
                    }
                });

                Results::with_errors(result, missing.chain(supplied).collect())
            })
    }

    fn transform_object_access(
        &mut self,
        ObjectAccess { expression, field }: ObjectAccess<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(expression).flat_map(|expression| {
            let PartialType::T(ty) = expression.expr.t.clone() else {
                return Results::ok(PartiallyTypedExpression {
                    t: PartialType::Unknown,
                    expression: Expression::new_object_access(expression, field),
                });
            };

            let Type::Object { fields } = ty.clone() else {
                return Results::with_error(
                    PartiallyTypedExpression {
                        t: PartialType::Unknown,
                        expression: Expression::new_object_access(expression, field.clone()),
                    },
                    SemanticError::InvalidFieldAccess {
                        object: PartialType::T(ty.clone()),
                        field: field.clone(),
                    }
                    .with_span(span),
                );
            };

            if let Some(t) = fields.get(&field) {
                Results::ok(PartiallyTypedExpression {
                    t: t.clone(),
                    expression: Expression::new_object_access(expression, field.clone()),
                })
            } else {
                Results::with_error(
                    PartiallyTypedExpression {
                        t: PartialType::Unknown,
                        expression: Expression::new_object_access(expression, field.clone()),
                    },
                    SemanticError::InvalidFieldAccess {
                        object: PartialType::T(ty.clone()),
                        field: field.clone(),
                    }
                    .with_span(span),
                )
            }
        })
    }
}
