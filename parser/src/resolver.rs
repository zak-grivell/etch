use std::collections::BTreeMap;

use ast::*;

use crate::parser::ParsedNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverError {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Symbol {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolNode;

impl Ast for SymbolNode {
    type Node<T> = AstNode<T, Self>;
    type Expression = Self::Node<Expression<Self>>;
    type Pattern = Self::Node<Pattern<Self>>;
    type Type = Self::Node<Type<Self>>;
    type Statement = Self::Node<Statement<Self>>;
    type Ident = Option<Symbol>;
    type U = UnaryOperator;
    type B = BinaryOperator;
    type Meta = Span;
}

pub struct SymbolResolver {
    scopes: Vec<BTreeMap<String, Symbol>>,
    next_symbol: u32,
}

impl Default for SymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolResolver {
    pub fn new() -> Self {
        let builtin_names = [
            "hook",
            "sim",
            "use_node",
            "use_state",
            "use_equation",
            "voltage",
            "conductance",
            "current",
            "fix_voltage",
            "drive_voltage",
            "time",
            "delta_time",
        ];
        let builtins = builtin_names
            .into_iter()
            .enumerate()
            .map(|(id, name)| {
                (
                    name.into(),
                    Symbol {
                        id: id as u32,
                        name: name.into(),
                    },
                )
            })
            .collect();
        Self {
            scopes: vec![builtins],
            next_symbol: (builtin_names.len() - 1) as u32,
        }
    }

    fn declare(&mut self, name: String) -> Symbol {
        self.next_symbol += 1;
        let symbol = Symbol {
            id: self.next_symbol,
            name: name.clone(),
        };
        self.scopes
            .last_mut()
            .expect("resolver always has a scope")
            .insert(name, symbol.clone());
        symbol
    }

    fn lookup(&self, name: &str) -> Option<Symbol> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
    }

    fn binding_pattern(
        &mut self,
        AstNode { inner, meta }: AstNode<Pattern<ParsedNode>, ParsedNode>,
    ) -> Results<AstNode<Pattern<SymbolNode>, SymbolNode>, ResolverError> {
        match inner {
            Pattern::Binding(ident) => Results::ok(AstNode {
                inner: Pattern::Binding(Ident {
                    ident: Some(self.declare(ident.ident)),
                }),
                meta,
            }),
            Pattern::Object(object) => object
                .fields
                .into_iter()
                .map(|(name, pattern)| self.binding_pattern(pattern).map(|pattern| (name, pattern)))
                .collect::<Results<BTreeMap<_, _>, _>>()
                .map(|fields| AstNode {
                    inner: Pattern::Object(ObjectDestructure { fields }),
                    meta,
                }),
            Pattern::Array(array) => array
                .values
                .into_iter()
                .map(|pattern| self.binding_pattern(pattern))
                .collect::<Results<Vec<_>, _>>()
                .map(|values| AstNode {
                    inner: Pattern::Array(ArrayDestructure { values }),
                    meta,
                }),
            Pattern::Enum(value) => Results::ok(AstNode {
                inner: Pattern::Enum(value),
                meta,
            }),
            Pattern::Primative(value) => Results::ok(AstNode {
                inner: Pattern::Primative(value),
                meta,
            }),
        }
    }
}

#[ast::transformer]
impl AstTransform for SymbolResolver {
    type Error = ResolverError;
    type From = ParsedNode;
    type To = SymbolNode;

    fn transform_definition(
        &mut self,
        AstNode { inner, meta }: AstNode<Definition<Self::From>, Self::From>,
    ) -> Results<AstNode<Definition<Self::To>, Self::To>, Self::Error> {
        self.binding_pattern(inner.lhs)
            .zip(self.transform_expression(inner.rhs))
            .map(|(lhs, rhs)| AstNode {
                inner: Definition { lhs, rhs },
                meta,
            })
    }

    fn transform_type_definition(
        &mut self,
        AstNode { inner, meta }: AstNode<TypeDefinition<Self::From>, Self::From>,
    ) -> Results<AstNode<TypeDefinition<Self::To>, Self::To>, Self::Error> {
        let lhs = Some(self.declare(inner.lhs));
        self.transform_type(inner.rhs).map(|rhs| AstNode {
            inner: TypeDefinition { lhs, rhs },
            meta,
        })
    }

    fn transform_return(
        &mut self,
        AstNode { inner, meta }: AstNode<Return<Self::From>, Self::From>,
    ) -> Results<AstNode<Return<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(inner.value).map(|value| AstNode {
            inner: Return { value },
            meta,
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
                self.scopes.push(BTreeMap::new());
                let pattern = arm
                    .pattern
                    .map_or_else(|| Results::ok(None), |v| self.binding_pattern(v).map(Some));
                let condition = arm.condition.map_or_else(
                    || Results::ok(None),
                    |v| self.transform_expression(*v).map(|v| Some(Box::new(v))),
                );
                let result = self.transform_expression(*arm.result).map(Box::new);
                self.scopes.pop();
                pattern
                    .zip(condition)
                    .zip(result)
                    .map(|((pattern, condition), result)| MatchArm {
                        pattern,
                        condition,
                        result,
                    })
            })
            .collect::<Results<Vec<_>, _>>();
        on.zip(arms).map(|(on, arms)| AstNode {
            inner: Match {
                on: Box::new(on),
                arms,
            },
            meta,
        })
    }

    fn transform_ident(
        &mut self,
        AstNode { inner, meta }: AstNode<Ident<Self::From>, Self::From>,
    ) -> Results<AstNode<Ident<Self::To>, Self::To>, Self::Error> {
        let symbol = self.lookup(&inner.ident);
        let output = AstNode {
            inner: Ident {
                ident: symbol.clone(),
            },
            meta,
        };
        match symbol {
            Some(_) => Results::ok(output),
            None => Results::with_error(
                output,
                ResolverError {
                    span: meta,
                    message: format!("undefined name `{}`", inner.ident),
                },
            ),
        }
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
            .map(|items| AstNode {
                inner: Array { items },
                meta,
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
            .map(|fields| AstNode {
                inner: Object { fields },
                meta,
            })
    }

    fn transform_lambda(
        &mut self,
        AstNode { inner, meta }: AstNode<Lambda<Self::From>, Self::From>,
    ) -> Results<AstNode<Lambda<Self::To>, Self::To>, Self::Error> {
        self.scopes.push(BTreeMap::new());
        let params = inner
            .params
            .into_iter()
            .map(|(name, ty)| {
                let symbol = self.declare(name);
                self.transform_type(AstNode { inner: ty, meta })
                    .map(|ty| (Some(symbol), ty.inner))
            })
            .collect::<Results<BTreeMap<_, _>, _>>();
        let body = self.transform_expression(*inner.body).map(Box::new);
        self.scopes.pop();
        params.zip(body).map(|(params, body)| AstNode {
            inner: Lambda { params, body },
            meta,
        })
    }

    fn transform_unary_op(
        &mut self,
        AstNode { inner, meta }: AstNode<UnaryOperation<Self::From>, Self::From>,
    ) -> Results<AstNode<UnaryOperation<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.arg).map(|arg| AstNode {
            inner: UnaryOperation {
                arg: Box::new(arg),
                op: inner.op,
            },
            meta,
        })
    }

    fn transform_binary_op(
        &mut self,
        AstNode { inner, meta }: AstNode<BinaryOperation<Self::From>, Self::From>,
    ) -> Results<AstNode<BinaryOperation<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.lhs)
            .zip(self.transform_expression(*inner.rhs))
            .map(|(lhs, rhs)| AstNode {
                inner: BinaryOperation {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    op: inner.op,
                },
                meta,
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
            .map(|(expression, args)| AstNode {
                inner: Call {
                    expression: Box::new(expression),
                    args,
                },
                meta,
            })
    }

    fn transform_object_access(
        &mut self,
        AstNode { inner, meta }: AstNode<ObjectAccess<Self::From>, Self::From>,
    ) -> Results<AstNode<ObjectAccess<Self::To>, Self::To>, Self::Error> {
        self.transform_expression(*inner.expression)
            .map(|expression| AstNode {
                inner: ObjectAccess {
                    expression: Box::new(expression),
                    field: inner.field,
                },
                meta,
            })
    }

    fn transform_node(
        &mut self,
        node: AstNode<Node, Self::From>,
    ) -> Results<AstNode<Node, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: node.inner,
            meta: node.meta,
        })
    }
    fn transform_object_pattern(
        &mut self,
        node: AstNode<ObjectDestructure<Self::From>, Self::From>,
    ) -> Results<AstNode<ObjectDestructure<Self::To>, Self::To>, Self::Error> {
        self.binding_pattern(node.map(Pattern::Object)).map(|node| {
            node.map(|p| match p {
                Pattern::Object(v) => v,
                _ => unreachable!(),
            })
        })
    }
    fn transform_array_pattern(
        &mut self,
        node: AstNode<ArrayDestructure<Self::From>, Self::From>,
    ) -> Results<AstNode<ArrayDestructure<Self::To>, Self::To>, Self::Error> {
        self.binding_pattern(node.map(Pattern::Array)).map(|node| {
            node.map(|p| match p {
                Pattern::Array(v) => v,
                _ => unreachable!(),
            })
        })
    }
    fn transform_enum_pattern(
        &mut self,
        node: AstNode<EnumDestructure, Self::From>,
    ) -> Results<AstNode<EnumDestructure, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: node.inner,
            meta: node.meta,
        })
    }
    fn transform_import(
        &mut self,
        AstNode { inner, meta }: AstNode<Import<Self::From>, Self::From>,
    ) -> Results<AstNode<Import<Self::To>, Self::To>, Self::Error> {
        self.binding_pattern(inner.imports).map(|imports| AstNode {
            inner: Import {
                imports,
                path: inner.path,
            },
            meta,
        })
    }

    fn transform_type_node(
        &mut self,
        n: AstNode<NodeType, Self::From>,
    ) -> Results<AstNode<NodeType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_type_number(
        &mut self,
        n: AstNode<NumberType, Self::From>,
    ) -> Results<AstNode<NumberType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_type_string(
        &mut self,
        n: AstNode<StringType, Self::From>,
    ) -> Results<AstNode<StringType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_type_boolean(
        &mut self,
        n: AstNode<BooleanType, Self::From>,
    ) -> Results<AstNode<BooleanType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_type_none(
        &mut self,
        n: AstNode<NoneType, Self::From>,
    ) -> Results<AstNode<NoneType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_type_never(
        &mut self,
        n: AstNode<NeverType, Self::From>,
    ) -> Results<AstNode<NeverType, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
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
            .map(|fields| AstNode {
                inner: ObjectType { fields },
                meta,
            })
    }
    fn transform_type_array(
        &mut self,
        AstNode { inner, meta }: AstNode<ArrayType<Self::From>, Self::From>,
    ) -> Results<AstNode<ArrayType<Self::To>, Self::To>, Self::Error> {
        self.transform_type(*inner.item_type)
            .map(|item_type| AstNode {
                inner: ArrayType {
                    item_type: Box::new(item_type),
                },
                meta,
            })
    }
    fn transform_type_optional(
        &mut self,
        AstNode { inner, meta }: AstNode<OptionalType<Self::From>, Self::From>,
    ) -> Results<AstNode<OptionalType<Self::To>, Self::To>, Self::Error> {
        self.transform_type(*inner.inner).map(|inner| AstNode {
            inner: OptionalType {
                inner: Box::new(inner),
            },
            meta,
        })
    }
    fn transform_type_lambda(
        &mut self,
        AstNode { inner, meta }: AstNode<LambdaType<Self::From>, Self::From>,
    ) -> Results<AstNode<LambdaType<Self::To>, Self::To>, Self::Error> {
        let params = inner
            .params
            .into_iter()
            .map(|(name, ty)| {
                self.transform_type(ty)
                    .map(|ty| (Some(self.declare(name)), ty))
            })
            .collect::<Results<BTreeMap<_, _>, _>>();
        params
            .zip(self.transform_type(*inner.rtn))
            .map(|(params, rtn)| AstNode {
                inner: LambdaType {
                    params,
                    rtn: Box::new(rtn),
                },
                meta,
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
            .map(|options| AstNode {
                inner: UnionType { options },
                meta,
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
            .map(|types| AstNode {
                inner: TupleType { types },
                meta,
            })
    }

    fn transform_string(
        &mut self,
        n: AstNode<String, Self::From>,
    ) -> Results<AstNode<String, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_boolean(
        &mut self,
        n: AstNode<bool, Self::From>,
    ) -> Results<AstNode<bool, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_number(
        &mut self,
        n: AstNode<f64, Self::From>,
    ) -> Results<AstNode<f64, Self::To>, Self::Error> {
        Results::ok(AstNode {
            inner: n.inner,
            meta: n.meta,
        })
    }
    fn transform_block(
        &mut self,
        AstNode { inner, meta }: AstNode<Block<Self::From>, Self::From>,
    ) -> Results<AstNode<Block<Self::To>, Self::To>, Self::Error> {
        self.scopes.push(BTreeMap::new());
        let body = inner
            .body
            .into_iter()
            .map(|v| self.transform_statement(v))
            .collect::<Results<Vec<_>, _>>();
        self.scopes.pop();
        body.map(|body| AstNode {
            inner: Block { body },
            meta,
        })
    }
}
