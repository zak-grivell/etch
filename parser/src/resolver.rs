use std::collections::{BTreeMap, BTreeSet};

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

// impl Default for SymbolResolver {
//     fn default() -> Self {
//         Self::new()
//     }
// }

impl SymbolResolver {
    pub fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
            next_symbol: 0,
        }
    }
}

//     fn metadata(meta: &ParsedMetadata) -> SymbolNode {
//         SymbolNode { span: meta.span }
//     }

//     fn declare(&mut self, name: String) -> Symbol {
//         self.next_symbol += 1;
//         let symbol = Symbol {
//             id: self.next_symbol,
//             name: name.clone(),
//         };
//         self.scopes
//             .last_mut()
//             .expect("symbol resolver always has a scope")
//             .insert(name, symbol.clone());
//         symbol
//     }

//     fn lookup(&self, name: &str) -> Option<Symbol> {
//         self.scopes
//             .iter()
//             .rev()
//             .find_map(|scope| scope.get(name))
//             .cloned()
//     }

//     fn transform_binding_pattern(
//         &mut self,
//         AstNode { inner, meta }: AstNode<Pattern<ParsedMetadata>, ParsedMetadata>,
//     ) -> Results<AstNode<Pattern<SymbolNode>, SymbolNode>, ResolverError> {
//         let output_meta = Self::metadata(&meta);
//         match inner {
            // Pattern::Binding(ident) => Results::ok(AstNode {
            //     inner: Pattern::Binding(Ident {
            //         ident: Some(self.declare(ident.ident)),
            //     }),
            //     meta: output_meta,
            // }),
//             Pattern::Object(object) => object
//                 .fields
//                 .into_iter()
//                 .map(|(name, pattern)| {
//                     self.transform_binding_pattern(pattern)
//                         .map(|pattern| (name, pattern))
//                 })
//                 .collect::<Results<BTreeMap<_, _>, _>>()
//                 .map(|fields| AstNode {
//                     inner: Pattern::Object(ObjectDestructure { fields }),
//                     meta: output_meta,
//                 }),
//             Pattern::Array(array) => array
//                 .values
//                 .into_iter()
//                 .map(|pattern| self.transform_binding_pattern(pattern))
//                 .collect::<Results<Vec<_>, _>>()
//                 .map(|values| AstNode {
//                     inner: Pattern::Array(ArrayDestructure { values }),
//                     meta: output_meta,
//                 }),
//             Pattern::Enum(value) => Results::ok(AstNode {
//                 inner: Pattern::Enum(value),
//                 meta: output_meta,
//             }),
//             Pattern::Primative(value) => Results::ok(AstNode {
//                 inner: Pattern::Primative(value),
//                 meta: output_meta,
//             }),
//         }
//     }

//     fn transform_raw_type(
//         &mut self,
//         inner: Type<ParsedMetadata>,
//         meta: ParsedMetadata,
//     ) -> Results<Type<SymbolNode>, ResolverError> {
//         self.transform_type(AstNode { inner, meta })
//             .map(|node| node.inner)
//     }

//     pub fn ok<T, U, V, W, E>(e: AstNode<T, U>) -> Results<AstNode<V, W>, E>
//     where
//         U: Ast,
//         T: Into<V>,
//         W: From<U> + Ast,
//     {
//         Results::ok(AstNode {
//             inner: e.inner.into(),
//             meta: W::from(e.meta),
//         })
//     }
// }

// impl Tr

impl Transform<Definition<ParsedNode>, Definition<SymbolNode>> for SymbolResolver {
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<Definition<ParsedNode>, Self::From>,
    ) -> Results<AstNode<Definition<SymbolNode>, Self::To>, Self::Error> {
        self.transform_pattern(inner.lhs)
            .zip(self.transform_expression(inner.rhs))
            .map(|(lhs, rhs)| AstNode {
                inner: Definition { lhs, rhs },
                meta,
            })
    }
}

impl Transform<ArrayDestructure<ParsedNode>, ArrayDestructure<ParsedNode>> for SymbolResolver 
    fn transform(
            &mut self,
            from: AstNode<ArrayDestructure<ParsedNode>, Self::From>,
    ) -> Results<AstNode<ArrayDestructure<ParsedNode>, Self::To>, Self::Error> {
        
    }
}

impl AstTransform for SymbolResolver {
    type Error = ResolverError;
    type From = ParsedNode;
    type To = SymbolNode;

    // fn transform_definition(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Definition<Self::From>, Self::From>,
    // ) -> Results<AstNode<Definition<Self::To>, Self::To>, Self::Error> {
    //     self.transform_binding_pattern(inner.lhs)
    //         .zip(self.transform_expression(inner.rhs))
    //         .map(|(lhs, rhs)| AstNode {
    //             inner: Definition { lhs, rhs },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_type_definition(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<TypeDefinition<Self::From>, Self::From>,
    // ) -> Results<AstNode<TypeDefinition<Self::To>, Self::To>, Self::Error> {
    //     let lhs = Some(self.declare(inner.lhs));
    //     self.transform_type(inner.rhs).map(|rhs| AstNode {
    //         inner: TypeDefinition { lhs, rhs },
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_return(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Return<Self::From>, Self::From>,
    // ) -> Results<AstNode<Return<Self::To>, Self::To>, Self::Error> {
    //     self.transform_expression(inner.value).map(|value| AstNode {
    //         inner: Return { value },
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_match(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Match<Self::From>, Self::From>,
    // ) -> Results<AstNode<Match<Self::To>, Self::To>, Self::Error> {
    //     let on = self.transform_expression(*inner.on);
    //     let arms = inner
    //         .arms
    //         .into_iter()
    //         .map(|arm| {
    //             self.scopes.push(BTreeMap::new());
    //             let pattern = match arm.pattern {
    //                 Some(pattern) => self.transform_binding_pattern(pattern).map(Some),
    //                 None => Results::ok(None),
    //             };
    //             let condition = match arm.condition {
    //                 Some(condition) => self
    //                     .transform_expression(*condition)
    //                     .map(|condition| Some(Box::new(condition))),
    //                 None => Results::ok(None),
    //             };
    //             let result = self.transform_expression(*arm.result).map(Box::new);
    //             self.scopes.pop();
    //             pattern
    //                 .zip(condition)
    //                 .zip(result)
    //                 .map(|((pattern, condition), result)| MatchArm {
    //                     pattern,
    //                     condition,
    //                     result,
    //                 })
    //         })
    //         .collect::<Results<Vec<_>, _>>();
    //     on.zip(arms).map(|(on, arms)| AstNode {
    //         inner: Match {
    //             on: Box::new(on),
    //             arms,
    //         },
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_ident(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Ident<Self::From>, Self::From>,
    // ) -> Results<AstNode<Ident<Self::To>, Self::To>, Self::Error> {
    //     let symbol = self.lookup(&inner.ident);
    //     let output = AstNode {
    //         inner: Ident {
    //             ident: symbol.clone(),
    //         },
    //         meta: Self::metadata(&meta),
    //     };
    //     match symbol {
    //         Some(_) => Results::ok(output),
    //         None => Results::with_error(
    //             output,
    //             ResolverError {
    //                 span: meta.span,
    //                 message: format!("undefined name `{}`", inner.ident),
    //             },
    //         ),
    //     }
    // }

    // fn transform_array(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Array<Self::From>, Self::From>,
    // ) -> Results<AstNode<Array<Self::To>, Self::To>, Self::Error> {
    //     inner
    //         .items
    //         .into_iter()
    //         .map(|item| self.transform_expression(item))
    //         .collect::<Results<Vec<_>, _>>()
    //         .map(|items| AstNode {
    //             inner: Array { items },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_object(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Object<Self::From>, Self::From>,
    // ) -> Results<AstNode<Object<Self::To>, Self::To>, Self::Error> {
    //     inner
    //         .fields
    //         .into_iter()
    //         .map(|(name, value)| self.transform_expression(value).map(|value| (name, value)))
    //         .collect::<Results<BTreeMap<_, _>, _>>()
    //         .map(|fields| AstNode {
    //             inner: Object { fields },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_lambda(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Lambda<Self::From>, Self::From>,
    // ) -> Results<AstNode<Lambda<Self::To>, Self::To>, Self::Error> {
    //     self.scopes.push(BTreeMap::new());
    //     let params = inner
    //         .params
    //         .into_iter()
    //         .map(|(name, ty)| {
    //             let symbol = self.declare(name);
    //             self.transform_raw_type(ty, meta.clone())
    //                 .map(|ty| (Some(symbol), ty))
    //         })
    //         .collect::<Results<BTreeMap<_, _>, _>>();
    //     let body = self.transform_expression(*inner.body).map(Box::new);
    //     self.scopes.pop();
    //     params.zip(body).map(|(params, body)| AstNode {
    //         inner: Lambda { params, body },
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_unary_op(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<UnaryOperation<Self::From>, Self::From>,
    // ) -> Results<AstNode<UnaryOperation<Self::To>, Self::To>, Self::Error> {
    //     self.transform_expression(*inner.arg).map(|arg| AstNode {
    //         inner: UnaryOperation {
    //             arg: Box::new(arg),
    //             op: inner.op,
    //         },
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_binary_op(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<BinaryOperation<Self::From>, Self::From>,
    // ) -> Results<AstNode<BinaryOperation<Self::To>, Self::To>, Self::Error> {
    //     self.transform_expression(*inner.lhs)
    //         .zip(self.transform_expression(*inner.rhs))
    //         .map(|(lhs, rhs)| AstNode {
    //             inner: BinaryOperation {
    //                 lhs: Box::new(lhs),
    //                 rhs: Box::new(rhs),
    //                 op: inner.op,
    //             },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_call(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Call<Self::From>, Self::From>,
    // ) -> Results<AstNode<Call<Self::To>, Self::To>, Self::Error> {
    //     let expression = self.transform_expression(*inner.expression).map(Box::new);
    //     let args = inner
    //         .args
    //         .into_iter()
    //         .map(|(name, value)| self.transform_expression(value).map(|value| (name, value)))
    //         .collect::<Results<BTreeMap<_, _>, _>>();
    //     expression.zip(args).map(|(expression, args)| AstNode {
    //         inner: Call { expression, args },
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_object_access(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<ObjectAccess<Self::From>, Self::From>,
    // ) -> Results<AstNode<ObjectAccess<Self::To>, Self::To>, Self::Error> {
    //     self.transform_expression(*inner.expression)
    //         .map(|expression| AstNode {
    //             inner: ObjectAccess {
    //                 expression: Box::new(expression),
    //                 field: inner.field,
    //             },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_node(
    //     &mut self,
    //     node: AstNode<Node, Self::From>,
    // ) -> Results<AstNode<Node, Self::To>, Self::Error> {
    //     Self::ok(node)
    // }

    // fn transform_object_pattern(
    //     &mut self,
    //     node: AstNode<ObjectDestructure<Self::From>, Self::From>,
    // ) -> Results<AstNode<ObjectDestructure<Self::To>, Self::To>, Self::Error> {
    //     self.transform_binding_pattern(node.map(Pattern::Object))
    //         .map(|node| {
    //             node.map(|pattern| match pattern {
    //                 Pattern::Object(object) => object,
    //                 _ => unreachable!(),
    //             })
    //         })
    // }

    // fn transform_array_pattern(
    //     &mut self,
    //     node: AstNode<ArrayDestructure<Self::From>, Self::From>,
    // ) -> Results<AstNode<ArrayDestructure<Self::To>, Self::To>, Self::Error> {
    //     self.transform_binding_pattern(node.map(Pattern::Array))
    //         .map(|node| {
    //             node.map(|pattern| match pattern {
    //                 Pattern::Array(array) => array,
    //                 _ => unreachable!(),
    //             })
    //         })
    // }

    // fn transform_enum_pattern(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<EnumDestructure, Self::From>,
    // ) -> Results<AstNode<EnumDestructure, Self::To>, Self::Error> {
    //     Results::ok(AstNode {
    //         inner,
    //         meta: Self::metadata(&meta),
    //     })
    // }

    // fn transform_import(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Import<Self::From>, Self::From>,
    // ) -> Results<AstNode<Import<Self::To>, Self::To>, Self::Error> {
    //     self.transform_binding_pattern(inner.imports)
    //         .map(|imports| AstNode {
    //             inner: Import {
    //                 imports,
    //                 path: inner.path,
    //             },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_type_node(
    //     &mut self,
    //     node: AstNode<NodeType, Self::From>,
    // ) -> Results<AstNode<NodeType, Self::To>, Self::Error> {
    //     self.transform(node)
    // }
    // fn transform_type_number(
    //     &mut self,
    //     node: AstNode<NumberType, Self::From>,
    // ) -> Results<AstNode<NumberType, Self::To>, Self::Error> {
    //     self.transform(node)
    // }
    // fn transform_type_string(
    //     &mut self,
    //     node: AstNode<StringType, Self::From>,
    // ) -> Results<AstNode<StringType, Self::To>, Self::Error> {
    //     self.transform(node)
    // }
    // fn transform_type_boolean(
    //     &mut self,
    //     node: AstNode<BooleanType, Self::From>,
    // ) -> Results<AstNode<BooleanType, Self::To>, Self::Error> {
    //     self.transform(node)
    // }
    // fn transform_type_none(
    //     &mut self,
    //     node: AstNode<NoneType, Self::From>,
    // ) -> Results<AstNode<NoneType, Self::To>, Self::Error> {
    //     self.transform(node)
    // }
    // fn transform_type_never(
    //     &mut self,
    //     node: AstNode<NeverType, Self::From>,
    // ) -> Results<AstNode<NeverType, Self::To>, Self::Error> {
    //     self.transform(node)
    // }

    // fn transform_type_object(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<ObjectType<Self::From>, Self::From>,
    // ) -> Results<AstNode<ObjectType<Self::To>, Self::To>, Self::Error> {
    //     inner
    //         .fields
    //         .into_iter()
    //         .map(|(name, ty)| self.transform_type(ty).map(|ty| (name, ty)))
    //         .collect::<Results<BTreeMap<_, _>, _>>()
    //         .map(|fields| AstNode {
    //             inner: ObjectType { fields },
    //             meta: Self::metadata(&meta),
    //         })
    // }
    // fn transform_type_array(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<ArrayType<Self::From>, Self::From>,
    // ) -> Results<AstNode<ArrayType<Self::To>, Self::To>, Self::Error> {
    //     self.transform_type(*inner.item_type)
    //         .map(|item_type| AstNode {
    //             inner: ArrayType {
    //                 item_type: Box::new(item_type),
    //             },
    //             meta: Self::metadata(&meta),
    //         })
    // }
    // fn transform_type_optional(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<OptionalType<Self::From>, Self::From>,
    // ) -> Results<AstNode<OptionalType<Self::To>, Self::To>, Self::Error> {
    //     self.transform_type(*inner.inner).map(|inner| AstNode {
    //         inner: OptionalType {
    //             inner: Box::new(inner),
    //         },
    //         meta: Self::metadata(&meta),
    //     })
    // }
    // fn transform_type_lambda(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<LambdaType<Self::From>, Self::From>,
    // ) -> Results<AstNode<LambdaType<Self::To>, Self::To>, Self::Error> {
    //     let params = inner
    //         .params
    //         .into_iter()
    //         .map(|(name, ty)| {
    //             self.transform_type(ty)
    //                 .map(|ty| (Some(self.declare(name)), ty))
    //         })
    //         .collect::<Results<BTreeMap<_, _>, _>>();
    //     params
    //         .zip(self.transform_type(*inner.rtn))
    //         .map(|(params, rtn)| AstNode {
    //             inner: LambdaType {
    //                 params,
    //                 rtn: Box::new(rtn),
    //             },
    //             meta: Self::metadata(&meta),
    //         })
    // }
    // fn transform_type_union(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<UnionType<Self::From>, Self::From>,
    // ) -> Results<AstNode<UnionType<Self::To>, Self::To>, Self::Error> {
    //     inner
    //         .options
    //         .into_iter()
    //         .map(|ty| self.transform_type(ty))
    //         .collect::<Results<Vec<_>, _>>()
    //         .map(|options| AstNode {
    //             inner: UnionType { options },
    //             meta: Self::metadata(&meta),
    //         })
    // }
    // fn transform_type_tuple(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<TupleType<Self::From>, Self::From>,
    // ) -> Results<AstNode<TupleType<Self::To>, Self::To>, Self::Error> {
    //     inner
    //         .types
    //         .into_iter()
    //         .map(|ty| self.transform_type(ty))
    //         .collect::<Results<Vec<_>, _>>()
    //         .map(|types| AstNode {
    //             inner: TupleType { types },
    //             meta: Self::metadata(&meta),
    //         })
    // }

    // fn transform_string(
    //     &mut self,
    //     node: AstNode<String, Self::From>,
    // ) -> Results<AstNode<String, Self::To>, Self::Error> {
    //     Self::ok(node)
    // }
    // fn transform_boolean(
    //     &mut self,
    //     node: AstNode<bool, Self::From>,
    // ) -> Results<AstNode<bool, Self::To>, Self::Error> {
    //     Self::ok(node)
    // }
    // fn transform_number(
    //     &mut self,
    //     node: AstNode<f64, Self::From>,
    // ) -> Results<AstNode<f64, Self::To>, Self::Error> {
    //     Self::ok(node)
    // }

    // fn transform_block(
    //     &mut self,
    //     AstNode { inner, meta }: AstNode<Block<Self::From>, Self::From>,
    // ) -> Results<AstNode<Block<Self::To>, Self::To>, Self::Error> {
    //     self.scopes.push(BTreeMap::new());
    //     let body = inner
    //         .body
    //         .into_iter()
    //         .map(|statement| self.transform_statement(statement))
    //         .collect::<Results<Vec<_>, _>>();
    //     self.scopes.pop();
    //     body.map(|body| AstNode {
    //         inner: Block { body },
    //         meta: Self::metadata(&meta),
    //     })
    // }
}
