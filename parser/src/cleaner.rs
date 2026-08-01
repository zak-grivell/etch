// use std::collections::BTreeMap;

// use ast::{
//     Ast, AstNode, AstTransform, BinaryOperation, BooleanLiteral, Call, Definition, Lambda, Match,
//     NumberLiteral, Object, ObjectAccess, PartialType, Results, Return, Span, StringLiteral,
//     StrongType, Symbol, Type, UnaryOperation,
// };
// use chumsky::{
//     error::Rich,
//     span::{SpanWrap, Spanned},
// };

// use crate::semantic::{PartialMetadata, TypedBinaryOperator, TypedUnaryOperator};

// #[derive(Debug, Clone, PartialEq)]
// pub struct StrongMetadata {
//     pub t: StrongType,
//     pub span: Span,
// }

// impl Ast for StrongMetadata {
//     type Expression = AstNode<Self>;
//     type Ident = Symbol;
//     type Type = StrongType;

//     type B = TypedBinaryOperator;
//     type U = TypedUnaryOperator;
// }

// #[derive(Debug, Clone)]
// pinner ub enum UnresolvedError {
//     Type,
//     Symbol,
//     Operator,
// }

// impl std::fmt::Display for UnresolvedError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             UnresolvedError::Type => write!(f, "unresolved type"),
//             UnresolvedError::Symbol => write!(f, "unresolved symbol"),
//             UnresolvedError::Operator => write!(f, "unresolved operator"),
//         }
//     }
// }

// impl UnresolvedError {
//     pub fn into_rich<'src>(self, span: Span) -> Rich<'src, String, Span> {
//         Rich::custom(span, format!("{self}"))
//     }
// }

// fn try_unwrap_type(t: PartialType, span: Span) -> Results<StrongType, Spanned<UnresolvedError>> {
//     match t.try_unwrap() {
//         Some(strong) => Results::ok(strong),
//         None => Results::with_error(
//             StrongType(Type::Never),
//             UnresolvedError::Type.with_span(span),
//         ),
//     }
// }

// fn strong_meta(t: PartialType, span: Span) -> Results<StrongMetadata, Spanned<UnresolvedError>> {
//     try_unwrap_type(t, span).map(|t| StrongMetadata { t, span })
// }

// pub struct ExpressionStripper;

// impl AstTransform for ExpressionStripper {
//     type Error = Spanned<UnresolvedError>;
//     type From = PartialMetadata;
//     type To = StrongMetadata;

//     fn transform_definition(
//         &mut self,
//         Definition { name, rhs }: Definition<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(Definition<Self::To>, Self::To), Self::Error> {
//         let name = match name {
//             Some(s) => Results::ok(s),
//             None => Results::with_error(
//                 Symbol {
//                     id: 0,
//                     original: String::new(),
//                 },
//                 UnresolvedError::Symbol.with_span(meta.span),
//             ),
//         };

//         self.transform(rhs)
//             .zip(strong_meta(meta.t, meta.span))
//             .zip(name)
//             .map(|((rhs, sm), name)| {
//                 (
//                     Definition { name, rhs },
//                     StrongMetadata {
//                         t: sm.t,
//                         span: meta.span,
//                     },
//                 )
//             })
//     }

//     fn transform_return(
//         &mut self,
//         Return { expression }: Return<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(Return<Self::To>, Self::To), Self::Error> {
//         self.transform(expression)
//             .zip(strong_meta(meta.t, meta.span))
//             .map(|(expression, sm)| {
//                 (
//                     Return { expression },
//                     StrongMetadata {
//                         t: sm.t,
//                         span: meta.span,
//                     },
//                 )
//             })
//     }

//     fn transform_match(
//         &mut self,
//         Match {
//             on: value,
//             arms: conds,
//         }: Match<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(Match<Self::To>, Self::To), Self::Error> {
//         self.transform(value)
//             .zip(
//                 conds
//                     .into_iter()
//                     .map(|(cond, body)| self.transform(cond).zip(self.transform(body)))
//                     .collect(),
//             )
//             .zip(strong_meta(meta.t, meta.span))
//             .map(|((value, conds), sm)| {
//                 (
//                     Match {
//                         on: value,
//                         arms: conds,
//                     },
//                     StrongMetadata {
//                         t: sm.t,
//                         span: meta.span,
//                     },
//                 )
//             })
//     }

//     fn transform_ident(
//         &mut self,
//         ident: ast::Ident<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(ast::Ident<Self::To>, Self::To), Self::Error> {
//         Results::ok((
//             ast::Ident {
//                 value: ident.value.unwrap(),
//             },
//             StrongMetadata {
//                 t: meta.t.unwrap(),
//                 span: meta.span,
//             },
//         ))
//     }

//     fn transform_number(
//         &mut self,
//         number: NumberLiteral,
//         meta: PartialMetadata,
//     ) -> Results<(NumberLiteral, Self::To), Self::Error> {
//         strong_meta(meta.t, meta.span).map(|sm| (number, sm))
//     }

//     fn transform_string(
//         &mut self,
//         string: StringLiteral,
//         meta: PartialMetadata,
//     ) -> Results<(StringLiteral, Self::To), Self::Error> {
//         strong_meta(meta.t, meta.span).map(|sm| (string, sm))
//     }

//     fn transform_boolean(
//         &mut self,
//         boolean: BooleanLiteral,
//         meta: PartialMetadata,
//     ) -> Results<(BooleanLiteral, Self::To), Self::Error> {
//         strong_meta(meta.t, meta.span).map(|sm| (boolean, sm))
//     }

//     fn transform_array(
//         &mut self,
//         array: ast::Array<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(ast::Array<Self::To>, Self::To), Self::Error> {
//         array
//             .items
//             .into_iter()
//             .map(|e| self.transform(e))
//             .collect::<Results<Vec<_>, _>>()
//             .zip(strong_meta(meta.t, meta.span))
//             .map(|(items, sm)| (ast::Array { items }, sm))
//     }

//     fn transform_object(
//         &mut self,
//         Object { fields }: Object<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(Object<Self::To>, Self::To), Self::Error> {
//         fields
//             .into_iter()
//             .map(|(name, e)| self.transform(e).map(|v| (name, v)))
//             .collect::<Results<BTreeMap<_, _>, _>>()
//             .zip(strong_meta(meta.t, meta.span))
//             .map(|(fields, sm)| (Object { fields }, sm))
//     }

//     fn transform_lambda(
//         &mut self,
//         lambda: Lambda<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(Lambda<Self::To>, Self::To), Self::Error> {
//         let params = lambda
//             .params
//             .into_iter()
//             .map(|(name, ty)| (name.unwrap(), ty.unwrap()))
//             .collect();

//         let body = lambda
//             .body
//             .into_iter()
//             .map(|e| self.transform(e))
//             .collect::<Results<Vec<_>, _>>();

//         body.zip(strong_meta(meta.t, meta.span))
//             .map(|(body, sm)| (Lambda { params, body }, sm))
//     }

//     fn transform_unary_op(
//         &mut self,
//         unary_operation: UnaryOperation<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(UnaryOperation<Self::To>, Self::To), Self::Error> {
//         self.transform(unary_operation.arg)
//             .zip(strong_meta(meta.t, meta.span))
//             .flat_map(|(arg, sm)| {
//                 let Some(op) = unary_operation.op else {
//                     return Results::with_error(
//                         (
//                             UnaryOperation {
//                                 arg,
//                                 op: TypedUnaryOperator::NegateNumber,
//                             },
//                             sm,
//                         ),
//                         UnresolvedError::Operator.with_span(meta.span),
//                     );
//                 };

//                 Results::ok((UnaryOperation { arg, op }, sm))
//             })
//     }

//     fn transform_binary_op(
//         &mut self,
//         binary_operation: BinaryOperation<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(BinaryOperation<Self::To>, Self::To), Self::Error> {
//         self.transform(binary_operation.lhs)
//             .zip(self.transform(binary_operation.rhs))
//             .zip(strong_meta(meta.t, meta.span))
//             .flat_map(|((lhs, rhs), sm)| {
//                 let Some(op) = binary_operation.op else {
//                     return Results::with_error(
//                         (
//                             BinaryOperation {
//                                 lhs,
//                                 rhs,
//                                 op: TypedBinaryOperator::AddNumbers,
//                             },
//                             sm,
//                         ),
//                         UnresolvedError::Operator.with_span(meta.span),
//                     );
//                 };

//                 Results::ok((BinaryOperation { lhs, rhs, op }, sm))
//             })
//     }

//     fn transform_call(
//         &mut self,
//         Call { expression, args }: Call<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(Call<Self::To>, Self::To), Self::Error> {
//         self.transform(expression)
//             .zip(
//                 args.into_iter()
//                     .map(|(name, e)| self.transform(e).map(|v| (name, v)))
//                     .collect::<Results<BTreeMap<_, _>, _>>(),
//             )
//             .zip(strong_meta(meta.t, meta.span))
//             .map(|((expression, args), sm)| (Call { expression, args }, sm))
//     }

//     fn transform_object_access(
//         &mut self,
//         ObjectAccess { expression, field }: ObjectAccess<Self::From>,
//         meta: PartialMetadata,
//     ) -> Results<(ObjectAccess<Self::To>, Self::To), Self::Error> {
//         self.transform(expression)
//             .zip(strong_meta(meta.t, meta.span))
//             .map(|(expression, sm)| (ObjectAccess { expression, field }, sm))
//     }

//     fn transform_node(
//         &mut self,
//         node: ast::AstNode,
//         meta: Self::From,
//     ) -> Results<(ast::Node, Self::To), Self::Error> {
//         strong_meta(meta.t, meta.span).map(|t| (node, t))
//     }
// }
