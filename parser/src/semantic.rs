use crate::lexer::Token;
use crate::resolver::SymbolMetadata;
use ast::{
    Ast, AstNode, AstTransform, BinaryOperation, BinaryOperator, Call, Definition, Expression,
    Lambda, Match, Object, ObjectAccess, PartialType, Results, Return, Span, Symbol, Type,
    UnaryOperation, UnaryOperator,
};
use chumsky::error::Rich;
use chumsky::span::{SpanWrap, Spanned};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct PartialMetadata {
    pub t: PartialType,
    pub span: Span,
}

impl Ast for PartialMetadata {
    type E = AstNode<Self>;
    type T = PartialType;

    type I = Option<Symbol>;

    type U = Option<TypedUnaryOperator>;

    type B = Option<TypedBinaryOperator>;
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

impl SemanticError {
    pub fn into_rich<'src>(self, span: Span) -> Rich<'src, String, Span> {
        Rich::custom(span, format!("{self}"))
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

impl AstTransform for TypeResolver {
    type Error = Spanned<SemanticError>;
    type From = SymbolMetadata;
    type To = PartialMetadata;

    fn transform_definition(
        &mut self,
        definition: Definition<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(Definition<Self::To>, Self::To), Self::Error> {
        let name = definition.name.clone().unwrap();
        self.transform(definition.rhs).map(|rhs| {
            let t = rhs.meta.t.clone();
            self.types.insert(name, t.clone());

            (
                Definition {
                    name: definition.name,
                    rhs,
                },
                PartialMetadata { t, span: meta.span },
            )
        })
    }

    fn transform_return(
        &mut self,
        rtn: Return<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(Return<Self::To>, Self::To), Self::Error> {
        self.transform(rtn.expression).map(|expression| {
            (
                Return { expression },
                PartialMetadata {
                    t: PartialType::T(Type::Never),
                    span: meta.span,
                },
            )
        })
    }

    fn transform_match(
        &mut self,
        mtch: Match<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(Match<Self::To>, Self::To), Self::Error> {
        let conds = mtch
            .conds
            .into_iter()
            .map(|(a, b)| self.transform(a).zip(self.transform(b)))
            .collect::<Results<Vec<_>, _>>();

        self.transform(mtch.value).zip(conds).map(|(value, conds)| {
            let t = PartialType::T(Type::Union {
                options: conds.iter().map(|(_, v)| v.meta.t.clone()).collect(),
            });
            (
                Match { value, conds },
                PartialMetadata { t, span: meta.span },
            )
        })
    }

    fn transform_ident(
        &mut self,
        ident: ast::Ident<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(ast::Ident<Self::To>, Self::To), Self::Error> {
        Results::ok((
            ast::Ident {
                value: ident.value.clone(),
            },
            PartialMetadata {
                t: if let Some(ident) = &ident.value {
                    self.types.get(ident).unwrap().clone()
                } else {
                    PartialType::Unknown
                },
                span: meta.span,
            },
        ))
    }

    fn transform_number(
        &mut self,
        number: ast::NumberLiteral,
        meta: SymbolMetadata,
    ) -> Results<(ast::NumberLiteral, Self::To), Self::Error> {
        Results::ok((
            number,
            PartialMetadata {
                t: PartialType::T(Type::Number),
                span: meta.span,
            },
        ))
    }

    fn transform_string(
        &mut self,
        string: ast::StringLiteral,
        meta: SymbolMetadata,
    ) -> Results<(ast::StringLiteral, Self::To), Self::Error> {
        Results::ok((
            string,
            PartialMetadata {
                t: PartialType::T(Type::String),
                span: meta.span,
            },
        ))
    }

    fn transform_boolean(
        &mut self,
        boolean: ast::BooleanLiteral,
        meta: SymbolMetadata,
    ) -> Results<(ast::BooleanLiteral, Self::To), Self::Error> {
        Results::ok((
            boolean,
            PartialMetadata {
                t: PartialType::T(Type::Boolean),
                span: meta.span,
            },
        ))
    }

    fn transform_array(
        &mut self,
        array: ast::Array<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(ast::Array<Self::To>, Self::To), Self::Error> {
        let items = array
            .items
            .into_iter()
            .map(|e| self.transform(e))
            .collect::<Results<Vec<_>, _>>();

        items.map(|items| {
            let elem = items
                .first()
                .map(|e| e.meta.t.clone())
                .unwrap_or(PartialType::Unknown);

            (
                ast::Array { items },
                PartialMetadata {
                    t: PartialType::T(Type::Array(Box::new(elem))),
                    span: meta.span,
                },
            )
        })
    }

    fn transform_object(
        &mut self,
        Object { fields }: Object<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(Object<Self::To>, Self::To), Self::Error> {
        let fields = fields
            .into_iter()
            .map(|(k, v)| self.transform(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>();

        fields.map(|fields| {
            let t = PartialType::T(Type::Object {
                fields: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.meta.t.clone()))
                    .collect(),
            });
            (Object { fields }, PartialMetadata { t, span: meta.span })
        })
    }

    fn transform_lambda(
        &mut self,
        lambda: Lambda<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(Lambda<Self::To>, Self::To), Self::Error> {
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
                resolve_multiple_types(body.iter().filter_map(|expr| match &*expr.expr {
                    Expression::Return(Return { expression }) => Some(expression.meta.t.clone()),
                    _ => None,
                }));

            let t = PartialType::T(Type::Lambda {
                params: lambda
                    .params
                    .iter()
                    .map(|(k, v)| (k.clone().unwrap(), v.clone()))
                    .collect(),
                rtn: Box::new(return_type),
            });

            (
                Lambda {
                    params: lambda.params,
                    body,
                },
                PartialMetadata { t, span: meta.span },
            )
        })
    }

    fn transform_unary_op(
        &mut self,
        UnaryOperation { arg, op }: UnaryOperation<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(UnaryOperation<Self::To>, Self::To), Self::Error> {
        self.transform(arg).flat_map(|arg| {
            let PartialType::T(ty) = arg.meta.t.clone() else {
                return Results::ok((
                    UnaryOperation { arg, op: None },
                    PartialMetadata {
                        t: PartialType::Unknown,
                        span: meta.span,
                    },
                ));
            };

            match (&ty, &op) {
                (Type::Number, UnaryOperator::Negate) => Results::ok((
                    UnaryOperation {
                        arg,
                        op: Some(TypedUnaryOperator::NegateNumber),
                    },
                    PartialMetadata {
                        t: PartialType::T(Type::Number),
                        span: meta.span,
                    },
                )),
                (Type::Boolean, UnaryOperator::Flip) => Results::ok((
                    UnaryOperation {
                        arg,
                        op: Some(TypedUnaryOperator::FlipBool),
                    },
                    PartialMetadata {
                        t: PartialType::T(Type::Boolean),
                        span: meta.span,
                    },
                )),
                (_, _) => Results::with_error(
                    (
                        UnaryOperation { arg, op: None },
                        PartialMetadata {
                            t: PartialType::Unknown,
                            span: meta.span,
                        },
                    ),
                    SemanticError::InvalidUrinaryOperation {
                        operator: op.clone(),
                        t: PartialType::T(ty.clone()),
                    }
                    .with_span(meta.span),
                ),
            }
        })
    }

    fn transform_binary_op(
        &mut self,
        BinaryOperation { lhs, rhs, op }: BinaryOperation<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(BinaryOperation<Self::To>, Self::To), Self::Error> {
        self.transform(lhs)
            .zip(self.transform(rhs))
            .flat_map(|(lhs, rhs)| {
                let PartialType::T(lhs_type) = lhs.meta.t.clone() else {
                    return Results::ok((
                        BinaryOperation { lhs, rhs, op: None },
                        PartialMetadata {
                            t: PartialType::Unknown,
                            span: meta.span,
                        },
                    ));
                };

                let PartialType::T(rhs_type) = rhs.meta.t.clone() else {
                    return Results::ok((
                        BinaryOperation { lhs, rhs, op: None },
                        PartialMetadata {
                            t: PartialType::Unknown,
                            span: meta.span,
                        },
                    ));
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
                        (
                            BinaryOperation { lhs, rhs, op: None },
                            PartialMetadata {
                                t: PartialType::Unknown,
                                span: meta.span,
                            },
                        ),
                        SemanticError::InvalidBinaryOperation {
                            operator: op.clone(),
                            left: PartialType::T(lhs_type.clone()),
                            right: PartialType::T(rhs_type.clone()),
                        }
                        .with_span(meta.span),
                    );
                };

                Results::ok((
                    BinaryOperation {
                        lhs,
                        rhs,
                        op: Some(typed_op),
                    },
                    PartialMetadata {
                        t: result_type,
                        span: meta.span,
                    },
                ))
            })
    }

    fn transform_call(
        &mut self,
        Call { expression, args }: Call<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(Call<Self::To>, Self::To), Self::Error> {
        let args = args
            .into_iter()
            .map(|(k, v)| self.transform(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>();

        args.zip(self.transform(expression))
            .flat_map(|(args, expression)| {
                let PartialType::T(ty) = expression.meta.t.clone() else {
                    return Results::ok((
                        Call { expression, args },
                        PartialMetadata {
                            t: PartialType::Unknown,
                            span: meta.span,
                        },
                    ));
                };

                let Type::Lambda { params, rtn } = ty.clone() else {
                    return Results::with_error(
                        (
                            Call {
                                expression,
                                args: args.clone(),
                            },
                            PartialMetadata {
                                t: PartialType::Unknown,
                                span: meta.span,
                            },
                        ),
                        SemanticError::TypeMismatch {
                            expected: PartialType::T(Type::Lambda {
                                params: args
                                    .iter()
                                    .map(|(k, v)| (Symbol::dummy(k.clone()), v.meta.t.clone()))
                                    .collect(),
                                rtn: Box::new(PartialType::Unknown),
                            }),
                            found: PartialType::T(ty),
                        }
                        .with_span(meta.span),
                    );
                };

                let result = (
                    Call {
                        expression,
                        args: args.clone(),
                    },
                    PartialMetadata {
                        t: (*rtn).clone(),
                        span: meta.span,
                    },
                );

                let required = params
                    .iter()
                    .map(|(k, v)| (k.original.clone(), v.clone()))
                    .collect::<BTreeMap<_, _>>();

                let missing = params
                    .iter()
                    .filter(|&(name, _)| !args.contains_key(&name.original))
                    .map(|(name, expected)| {
                        SemanticError::ArgumentMissing {
                            name: name.original.clone(),
                            t: expected.clone(),
                        }
                        .with_span(meta.span)
                    });

                let supplied = args.iter().filter_map(|(name, value)| {
                    let name = name.clone();

                    match required.get(&name) {
                        Some(expected) if *expected == value.meta.t => None,
                        Some(expected) => Some(
                            SemanticError::InvalidArgumentType {
                                name: name.clone(),
                                expected: expected.clone(),
                                got: value.meta.t.clone(),
                            }
                            .with_span(meta.span),
                        ),
                        None => Some(
                            SemanticError::ExtraArgument { name: name.clone() }
                                .with_span(meta.span),
                        ),
                    }
                });

                Results::with_errors(result, missing.chain(supplied).collect())
            })
    }

    fn transform_object_access(
        &mut self,
        ObjectAccess { expression, field }: ObjectAccess<Self::From>,
        meta: SymbolMetadata,
    ) -> Results<(ObjectAccess<Self::To>, Self::To), Self::Error> {
        self.transform(expression).flat_map(|expression| {
            let PartialType::T(ty) = expression.meta.t.clone() else {
                return Results::ok((
                    ObjectAccess { expression, field },
                    PartialMetadata {
                        t: PartialType::Unknown,
                        span: meta.span,
                    },
                ));
            };

            let Type::Object { fields } = ty.clone() else {
                return Results::with_error(
                    (
                        ObjectAccess {
                            expression,
                            field: field.clone(),
                        },
                        PartialMetadata {
                            t: PartialType::Unknown,
                            span: meta.span,
                        },
                    ),
                    SemanticError::InvalidFieldAccess {
                        object: PartialType::T(ty.clone()),
                        field: field.clone(),
                    }
                    .with_span(meta.span),
                );
            };

            if let Some(t) = fields.get(&field) {
                Results::ok((
                    ObjectAccess {
                        expression,
                        field: field.clone(),
                    },
                    PartialMetadata {
                        t: t.clone(),
                        span: meta.span,
                    },
                ))
            } else {
                Results::with_error(
                    (
                        ObjectAccess {
                            expression,
                            field: field.clone(),
                        },
                        PartialMetadata {
                            t: PartialType::Unknown,
                            span: meta.span,
                        },
                    ),
                    SemanticError::InvalidFieldAccess {
                        object: PartialType::T(ty.clone()),
                        field: field.clone(),
                    }
                    .with_span(meta.span),
                )
            }
        })
    }
}
