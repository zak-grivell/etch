use std::collections::BTreeMap;

use ast::*;

use crate::resolver::{Symbol, SymbolNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Unknown,
    Never,
    Node,
    Number,
    String,
    Boolean,
    None,
    Array(Box<Self>),
    Object(BTreeMap<String, Self>),
    Lambda {
        params: BTreeMap<String, Self>,
        rtn: Box<Self>,
    },
    Optional(Box<Self>),
    Tuple(Vec<Self>),
    Union(Vec<Self>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialMetadata {
    pub span: Span,
    pub ty: ValueType,
}

impl Ast for PartialMetadata {
    type Node<T> = AstNode<T, Self>;
    type Expression = Self::Node<Expression<Self>>;
    type Pattern = Self::Node<Pattern<Self>>;
    type Type = Self::Node<Type<Self>>;
    type Statement = Self::Node<Statement<Self>>;
    type Ident = Option<Symbol>;
    type U = UnaryOperator;
    type B = BinaryOperator;
    type Meta = Self;
}

pub type TypedProgram = AstNode<Program<PartialMetadata>, PartialMetadata>;

#[derive(Default)]
pub struct TypeResolver {
    types: BTreeMap<Symbol, ValueType>,
}

impl TypeResolver {
    pub fn new() -> Self {
        Self::default()
    }
    fn meta(meta: &Span, ty: ValueType) -> PartialMetadata {
        PartialMetadata { span: *meta, ty }
    }

    fn error(meta: &Span, message: impl Into<String>) -> SemanticError {
        SemanticError {
            span: *meta,
            message: message.into(),
        }
    }

    fn raw_type(
        &mut self,
        inner: Type<SymbolNode>,
        meta: Span,
    ) -> Results<Type<PartialMetadata>, SemanticError> {
        self.transform_type(AstNode { inner, meta })
            .map(|node| node.inner)
    }
    fn bind(
        &mut self,
        pattern: &AstNode<Pattern<PartialMetadata>, PartialMetadata>,
        ty: ValueType,
    ) {
        match &pattern.inner {
            Pattern::Binding(Ident {
                ident: Some(symbol),
            }) => {
                self.types.insert(symbol.clone(), ty);
            }
            Pattern::Object(object) => {
                for (name, pattern) in &object.fields {
                    self.bind(
                        pattern,
                        match &ty {
                            ValueType::Object(fields) => {
                                fields.get(name).cloned().unwrap_or(ValueType::Unknown)
                            }
                            _ => ValueType::Unknown,
                        },
                    );
                }
            }
            Pattern::Array(array) => {
                for pattern in &array.values {
                    self.bind(
                        pattern,
                        match &ty {
                            ValueType::Array(item) => (**item).clone(),
                            _ => ValueType::Unknown,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

fn compatible(a: &ValueType, b: &ValueType) -> bool {
    a == b || *a == ValueType::Unknown || *b == ValueType::Unknown
}
fn unify(types: impl IntoIterator<Item = ValueType>) -> ValueType {
    let mut unique = Vec::new();
    for ty in types {
        if !unique.contains(&ty) {
            unique.push(ty);
        }
    }
    match unique.len() {
        0 => ValueType::None,
        1 => unique.pop().unwrap(),
        _ => ValueType::Union(unique),
    }
}

#[ast::transformer]
impl AstTransform for TypeResolver {
    type Error = SemanticError;
    type From = SymbolNode;
    type To = PartialMetadata;

    fn transform_definition(
        &mut self,
        AstNode { inner, meta }: AstNode<Definition<Self::From>, Self::From>,
    ) -> Results<AstNode<Definition<Self::To>, Self::To>, Self::Error> {
        self.transform_pattern(inner.lhs)
            .zip(self.transform_expression(inner.rhs))
            .map(|(lhs, rhs)| {
                let ty = rhs.meta.ty.clone();
                self.bind(&lhs, ty.clone());
                AstNode {
                    inner: Definition { lhs, rhs },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_type_definition(
        &mut self,
        AstNode { inner, meta }: AstNode<TypeDefinition<Self::From>, Self::From>,
    ) -> Results<AstNode<TypeDefinition<Self::To>, Self::To>, Self::Error> {
        self.transform_type(inner.rhs).map(|rhs| AstNode {
            inner: TypeDefinition {
                lhs: inner.lhs,
                rhs,
            },
            meta: Self::meta(&meta, ValueType::None),
        })
    }
    fn transform_return(
        &mut self,
        AstNode { inner, meta }: AstNode<Return<Self::From>, Self::From>,
    ) -> Results<AstNode<Return<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(inner.value).map(|value| AstNode {
            inner: Return { value },
            meta: Self::meta(&meta, ValueType::Never),
        })
    }
    fn transform_match(
        &mut self,
        AstNode { inner, meta }: AstNode<Match<Self::From>, Self::From>,
    ) -> Results<AstNode<Match<Self::To>, Self::To>, Self::Error> {
        let on = self.transform_expression(*inner.on);
        let arms = inner
            .arms
            .into_iter()
            .map(|arm| {
                let pattern = match arm.pattern {
                    Some(v) => self.transform_pattern(v).map(Some),
                    None => Results::ok(None),
                };
                let condition = match arm.condition {
                    Some(v) => self.transform_expression(*v).flat_map(|v| {
                        let errors = if compatible(&ValueType::Boolean, &v.meta.ty) {
                            vec![]
                        } else {
                            vec![Self::error(&meta, "match guard must be boolean")]
                        };
                        Results::with_errors(Some(Box::new(v)), errors)
                    }),
                    None => Results::ok(None),
                };
                pattern
                    .zip(condition)
                    .zip(self.transform_expression(*arm.result).map(Box::new))
                    .map(|((pattern, condition), result)| MatchArm {
                        pattern,
                        condition,
                        result,
                    })
            })
            .collect::<Results<Vec<MatchArm<PartialMetadata>>, _>>();
        on.zip(arms).map(|(on, arms)| {
            let ty = unify(arms.iter().map(|arm| arm.result.meta.ty.clone()));
            AstNode {
                inner: Match {
                    on: Box::new(on),
                    arms,
                },
                meta: Self::meta(&meta, ty),
            }
        })
    }
    fn transform_ident(
        &mut self,
        AstNode { inner, meta }: AstNode<Ident<Self::From>, Self::From>,
    ) -> Results<AstNode<Ident<Self::To>, Self::To>, Self::Error> {
        let ty = inner
            .ident
            .as_ref()
            .and_then(|s| self.types.get(s))
            .cloned()
            .unwrap_or(ValueType::Unknown);
        Results::ok(AstNode {
            inner: Ident { ident: inner.ident },
            meta: Self::meta(&meta, ty),
        })
    }
    fn transform_array(
        &mut self,
        AstNode { inner, meta }: AstNode<Array<Self::From>, Self::From>,
    ) -> Results<AstNode<Array<Self::To>, Self::To>, Self::Error> {
        inner
            .items
            .into_iter()
            .map(|v| self.transform_expression(v))
            .collect::<Results<Vec<_>, _>>()
            .flat_map(|items| {
                let item_ty = items
                    .first()
                    .map(|v| v.meta.ty.clone())
                    .unwrap_or(ValueType::Unknown);
                let errors = if items.iter().all(|v| compatible(&item_ty, &v.meta.ty)) {
                    vec![]
                } else {
                    vec![Self::error(&meta, "array items must have the same type")]
                };
                Results::with_errors(
                    AstNode {
                        inner: Array { items },
                        meta: Self::meta(&meta, ValueType::Array(Box::new(item_ty))),
                    },
                    errors,
                )
            })
    }
    fn transform_object(
        &mut self,
        AstNode { inner, meta }: AstNode<Object<Self::From>, Self::From>,
    ) -> Results<AstNode<Object<Self::To>, Self::To>, Self::Error> {
        inner
            .fields
            .into_iter()
            .map(|(k, v)| self.transform_expression(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>()
            .map(|fields| {
                let ty = ValueType::Object(
                    fields
                        .iter()
                        .map(|(k, v)| (k.clone(), v.meta.ty.clone()))
                        .collect(),
                );
                AstNode {
                    inner: Object { fields },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_lambda(
        &mut self,
        AstNode { inner, meta }: AstNode<Lambda<Self::From>, Self::From>,
    ) -> Results<AstNode<Lambda<Self::To>, Self::To>, Self::Error> {
        let params = inner
            .params
            .into_iter()
            .map(|(symbol, ty)| {
                self.raw_type(ty, meta).map(|ty| {
                    if let Some(s) = &symbol {
                        self.types.insert(s.clone(), type_value(&ty));
                    }
                    (symbol, ty)
                })
            })
            .collect::<Results<BTreeMap<_, _>, _>>();
        params
            .zip(self.transform_expression(*inner.body).map(Box::new))
            .map(|(params, body)| {
                let ty = ValueType::Lambda {
                    params: params
                        .iter()
                        .filter_map(|(s, t)| s.as_ref().map(|s| (s.name.clone(), type_value(t))))
                        .collect(),
                    rtn: Box::new(body.meta.ty.clone()),
                };
                AstNode {
                    inner: Lambda { params, body },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_unary_op(
        &mut self,
        AstNode { inner, meta }: AstNode<UnaryOperation<Self::From>, Self::From>,
    ) -> Results<AstNode<UnaryOperation<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.arg).flat_map(|arg| {
            let ty = match inner.op {
                UnaryOperator::Negate if compatible(&ValueType::Number, &arg.meta.ty) => {
                    ValueType::Number
                }
                UnaryOperator::Flip if compatible(&ValueType::Boolean, &arg.meta.ty) => {
                    ValueType::Boolean
                }
                _ => ValueType::Unknown,
            };
            let errors = if ty == ValueType::Unknown && arg.meta.ty != ValueType::Unknown {
                vec![Self::error(&meta, "invalid operand for unary operator")]
            } else {
                vec![]
            };
            Results::with_errors(
                AstNode {
                    inner: UnaryOperation {
                        arg: Box::new(arg),
                        op: inner.op,
                    },
                    meta: Self::meta(&meta, ty),
                },
                errors,
            )
        })
    }
    fn transform_binary_op(
        &mut self,
        AstNode { inner, meta }: AstNode<BinaryOperation<Self::From>, Self::From>,
    ) -> Results<AstNode<BinaryOperation<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.lhs)
            .zip(self.transform_expression(*inner.rhs))
            .flat_map(|(lhs, rhs)| {
                let ty = match inner.op {
                    BinaryOperator::Equal if compatible(&lhs.meta.ty, &rhs.meta.ty) => {
                        ValueType::Boolean
                    }
                    BinaryOperator::LessThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::LessThanOrEqual
                    | BinaryOperator::GreaterThanOrEqual
                        if compatible(&ValueType::Number, &lhs.meta.ty)
                            && compatible(&ValueType::Number, &rhs.meta.ty) =>
                    {
                        ValueType::Boolean
                    }
                    BinaryOperator::Add
                    | BinaryOperator::Sub
                    | BinaryOperator::Mul
                    | BinaryOperator::Div
                        if compatible(&ValueType::Number, &lhs.meta.ty)
                            && compatible(&ValueType::Number, &rhs.meta.ty) =>
                    {
                        ValueType::Number
                    }
                    BinaryOperator::Wire
                        if compatible(&ValueType::Node, &lhs.meta.ty)
                            && compatible(&ValueType::Node, &rhs.meta.ty) =>
                    {
                        ValueType::Node
                    }
                    BinaryOperator::Union => unify([lhs.meta.ty.clone(), rhs.meta.ty.clone()]),
                    _ => ValueType::Unknown,
                };
                let errors = if ty == ValueType::Unknown
                    && lhs.meta.ty != ValueType::Unknown
                    && rhs.meta.ty != ValueType::Unknown
                {
                    vec![Self::error(&meta, "invalid operands for binary operator")]
                } else {
                    vec![]
                };
                Results::with_errors(
                    AstNode {
                        inner: BinaryOperation {
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                            op: inner.op,
                        },
                        meta: Self::meta(&meta, ty),
                    },
                    errors,
                )
            })
    }
    fn transform_call(
        &mut self,
        AstNode { inner, meta }: AstNode<Call<Self::From>, Self::From>,
    ) -> Results<AstNode<Call<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.expression)
            .zip(
                inner
                    .args
                    .into_iter()
                    .map(|(k, v)| self.transform_expression(v).map(|v| (k, v)))
                    .collect::<Results<BTreeMap<_, _>, _>>(),
            )
            .flat_map(|(expression, args)| {
                let mut errors = Vec::new();
                let ty = match &expression.meta.ty {
                    ValueType::Lambda { params, rtn } => {
                        for (name, expected) in params {
                            match args.get(name) {
                                Some(actual) if compatible(expected, &actual.meta.ty) => {}
                                Some(_) => errors.push(Self::error(
                                    &meta,
                                    format!("argument `{name}` has the wrong type"),
                                )),
                                None => errors
                                    .push(Self::error(&meta, format!("missing argument `{name}`"))),
                            }
                        }
                        for name in args.keys() {
                            if !params.contains_key(name) {
                                errors.push(Self::error(
                                    &meta,
                                    format!("unexpected argument `{name}`"),
                                ))
                            }
                        }
                        (**rtn).clone()
                    }
                    ValueType::Unknown => ValueType::Unknown,
                    _ => {
                        errors.push(Self::error(&meta, "only lambdas can be called"));
                        ValueType::Unknown
                    }
                };
                Results::with_errors(
                    AstNode {
                        inner: Call {
                            expression: Box::new(expression),
                            args,
                        },
                        meta: Self::meta(&meta, ty),
                    },
                    errors,
                )
            })
    }
    fn transform_object_access(
        &mut self,
        AstNode { inner, meta }: AstNode<ObjectAccess<Self::From>, Self::From>,
    ) -> Results<AstNode<ObjectAccess<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.expression)
            .flat_map(|expression| {
                let (ty, errors) = match &expression.meta.ty {
                    ValueType::Object(fields) => match fields.get(&inner.field) {
                        Some(v) => (v.clone(), vec![]),
                        None => (
                            ValueType::Unknown,
                            vec![Self::error(
                                &meta,
                                format!("object has no field `{}`", inner.field),
                            )],
                        ),
                    },
                    ValueType::Unknown => (ValueType::Unknown, vec![]),
                    _ => (
                        ValueType::Unknown,
                        vec![Self::error(&meta, "field access requires an object")],
                    ),
                };
                Results::with_errors(
                    AstNode {
                        inner: ObjectAccess {
                            expression: Box::new(expression),
                            field: inner.field,
                        },
                        meta: Self::meta(&meta, ty),
                    },
                    errors,
                )
            })
    }
    fn transform_node(
        &mut self,
        AstNode { inner, meta }: AstNode<Node, Self::From>,
    ) -> Results<AstNode<Node, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner,
            meta: Self::meta(&meta, ValueType::Node),
        })
    }
    fn transform_object_pattern(
        &mut self,
        AstNode { inner, meta }: AstNode<ObjectDestructure<Self::From>, Self::From>,
    ) -> Results<AstNode<ObjectDestructure<Self::To>, Self::To>, Self::Error> {
        inner
            .fields
            .into_iter()
            .map(|(k, v)| self.transform_pattern(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>()
            .map(|fields| AstNode {
                inner: ObjectDestructure { fields },
                meta: Self::meta(&meta, ValueType::Unknown),
            })
    }
    fn transform_array_pattern(
        &mut self,
        AstNode { inner, meta }: AstNode<ArrayDestructure<Self::From>, Self::From>,
    ) -> Results<AstNode<ArrayDestructure<Self::To>, Self::To>, Self::Error> {
        inner
            .values
            .into_iter()
            .map(|v| self.transform_pattern(v))
            .collect::<Results<Vec<_>, _>>()
            .map(|values| AstNode {
                inner: ArrayDestructure { values },
                meta: Self::meta(&meta, ValueType::Unknown),
            })
    }
    fn transform_enum_pattern(
        &mut self,
        AstNode { inner, meta }: AstNode<EnumDestructure, Self::From>,
    ) -> Results<AstNode<EnumDestructure, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner,
            meta: Self::meta(&meta, ValueType::Unknown),
        })
    }
    fn transform_import(
        &mut self,
        AstNode { inner, meta }: AstNode<Import<Self::From>, Self::From>,
    ) -> Results<AstNode<Import<Self::To>, Self::To>, Self::Error> {
        self.transform_pattern(inner.imports)
            .map(|imports| AstNode {
                inner: Import {
                    imports,
                    path: inner.path,
                },
                meta: Self::meta(&meta, ValueType::None),
            })
    }
    fn transform_type_node(
        &mut self,
        n: AstNode<NodeType, Self::From>,
    ) -> Results<AstNode<NodeType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::Node),
        })
    }
    fn transform_type_number(
        &mut self,
        n: AstNode<NumberType, Self::From>,
    ) -> Results<AstNode<NumberType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::Number),
        })
    }
    fn transform_type_string(
        &mut self,
        n: AstNode<StringType, Self::From>,
    ) -> Results<AstNode<StringType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::String),
        })
    }
    fn transform_type_boolean(
        &mut self,
        n: AstNode<BooleanType, Self::From>,
    ) -> Results<AstNode<BooleanType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::Boolean),
        })
    }
    fn transform_type_none(
        &mut self,
        n: AstNode<NoneType, Self::From>,
    ) -> Results<AstNode<NoneType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::None),
        })
    }
    fn transform_type_never(
        &mut self,
        n: AstNode<NeverType, Self::From>,
    ) -> Results<AstNode<NeverType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::Never),
        })
    }
    fn transform_type_object(
        &mut self,
        AstNode { inner, meta }: AstNode<ObjectType<Self::From>, Self::From>,
    ) -> Results<AstNode<ObjectType<Self::To>, Self::To>, Self::Error> {
        inner
            .fields
            .into_iter()
            .map(|(k, v)| self.transform_type(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>()
            .map(|fields| {
                let ty = ValueType::Object(
                    fields
                        .iter()
                        .map(|(k, v)| (k.clone(), v.meta.ty.clone()))
                        .collect(),
                );
                AstNode {
                    inner: ObjectType { fields },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_type_array(
        &mut self,
        AstNode { inner, meta }: AstNode<ArrayType<Self::From>, Self::From>,
    ) -> Results<AstNode<ArrayType<Self::To>, Self::To>, Self::Error> {
        self.transform_type(*inner.item_type).map(|item_type| {
            let ty = ValueType::Array(Box::new(item_type.meta.ty.clone()));
            AstNode {
                inner: ArrayType {
                    item_type: Box::new(item_type),
                },
                meta: Self::meta(&meta, ty),
            }
        })
    }
    fn transform_type_optional(
        &mut self,
        AstNode { inner, meta }: AstNode<OptionalType<Self::From>, Self::From>,
    ) -> Results<AstNode<OptionalType<Self::To>, Self::To>, Self::Error> {
        self.transform_type(*inner.inner).map(|inner| {
            let ty = ValueType::Optional(Box::new(inner.meta.ty.clone()));
            AstNode {
                inner: OptionalType {
                    inner: Box::new(inner),
                },
                meta: Self::meta(&meta, ty),
            }
        })
    }
    fn transform_type_lambda(
        &mut self,
        AstNode { inner, meta }: AstNode<LambdaType<Self::From>, Self::From>,
    ) -> Results<AstNode<LambdaType<Self::To>, Self::To>, Self::Error> {
        inner
            .params
            .into_iter()
            .map(|(k, v)| self.transform_type(v).map(|v| (k, v)))
            .collect::<Results<BTreeMap<_, _>, _>>()
            .zip(self.transform_type(*inner.rtn))
            .map(|(params, rtn)| {
                let ty = ValueType::Lambda {
                    params: params
                        .iter()
                        .filter_map(|(k, v)| {
                            k.as_ref().map(|k| (k.name.clone(), v.meta.ty.clone()))
                        })
                        .collect(),
                    rtn: Box::new(rtn.meta.ty.clone()),
                };
                AstNode {
                    inner: LambdaType {
                        params,
                        rtn: Box::new(rtn),
                    },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_type_union(
        &mut self,
        AstNode { inner, meta }: AstNode<UnionType<Self::From>, Self::From>,
    ) -> Results<AstNode<UnionType<Self::To>, Self::To>, Self::Error> {
        inner
            .options
            .into_iter()
            .map(|v| self.transform_type(v))
            .collect::<Results<Vec<_>, _>>()
            .map(|options| {
                let ty = ValueType::Union(options.iter().map(|v| v.meta.ty.clone()).collect());
                AstNode {
                    inner: UnionType { options },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_type_tuple(
        &mut self,
        AstNode { inner, meta }: AstNode<TupleType<Self::From>, Self::From>,
    ) -> Results<AstNode<TupleType<Self::To>, Self::To>, Self::Error> {
        inner
            .types
            .into_iter()
            .map(|v| self.transform_type(v))
            .collect::<Results<Vec<_>, _>>()
            .map(|types| {
                let ty = ValueType::Tuple(types.iter().map(|v| v.meta.ty.clone()).collect());
                AstNode {
                    inner: TupleType { types },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
    fn transform_string(
        &mut self,
        n: AstNode<String, Self::From>,
    ) -> Results<AstNode<String, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::String),
        })
    }
    fn transform_boolean(
        &mut self,
        n: AstNode<bool, Self::From>,
    ) -> Results<AstNode<bool, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::Boolean),
        })
    }
    fn transform_number(
        &mut self,
        n: AstNode<f64, Self::From>,
    ) -> Results<AstNode<f64, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: Self::meta(&n.meta, ValueType::Number),
        })
    }
    fn transform_block(
        &mut self,
        AstNode { inner, meta }: AstNode<Block<Self::From>, Self::From>,
    ) -> Results<AstNode<Block<Self::To>, Self::To>, Self::Error> {
        inner
            .body
            .into_iter()
            .map(|v| self.transform_statement(v))
            .collect::<Results<Vec<_>, _>>()
            .map(|body| {
                let ty = body
                    .last()
                    .map(|v| v.meta.ty.clone())
                    .unwrap_or(ValueType::None);
                AstNode {
                    inner: Block { body },
                    meta: Self::meta(&meta, ty),
                }
            })
    }
}

fn type_value(ty: &Type<PartialMetadata>) -> ValueType {
    match ty {
        Type::Node(_) => ValueType::Node,
        Type::Number(_) => ValueType::Number,
        Type::String(_) => ValueType::String,
        Type::Boolean(_) => ValueType::Boolean,
        Type::None(_) => ValueType::None,
        Type::Never(_) => ValueType::Never,
        Type::Object(v) => ValueType::Object(
            v.fields
                .iter()
                .map(|(k, v)| (k.clone(), v.meta.ty.clone()))
                .collect(),
        ),
        Type::Array(v) => ValueType::Array(Box::new(v.item_type.meta.ty.clone())),
        Type::Optional(v) => ValueType::Optional(Box::new(v.inner.meta.ty.clone())),
        Type::Lambda(v) => ValueType::Lambda {
            params: v
                .params
                .iter()
                .filter_map(|(k, v)| k.as_ref().map(|k| (k.name.clone(), v.meta.ty.clone())))
                .collect(),
            rtn: Box::new(v.rtn.meta.ty.clone()),
        },
        Type::Union(v) => ValueType::Union(v.options.iter().map(|v| v.meta.ty.clone()).collect()),
        Type::Tuple(v) => ValueType::Tuple(v.types.iter().map(|v| v.meta.ty.clone()).collect()),
    }
}
