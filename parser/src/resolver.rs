use std::{collections::BTreeMap, fmt::Display};

use ast::{
    Array, Ast, AstNode, AstTransform, BinaryOperation, BinaryOperator, BooleanLiteral, Call,
    Definition, Ident, Lambda, NumberLiteral, Object, ObjectAccess, PartialType, Results, Span,
    StringLiteral, Symbol, UnaryOperation, UnaryOperator,
};
use chumsky::{
    error::Rich,
    span::{SpanWrap, Spanned},
};

use crate::{lexer::Token, parser::ParsedMetadata};

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolMetadata {
    pub span: Span,
}

impl Ast for SymbolMetadata {
    type E = AstNode<Self>;
    type I = Option<Symbol>;
    type T = PartialType;

    type B = BinaryOperator;
    type U = UnaryOperator;
}

#[derive(Debug, Clone)]
pub enum SymbolicError {
    Undefined { name: String },
}

impl Display for SymbolicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolicError::Undefined { name } => write!(f, "{} is not defined", name),
        }
    }
}

impl SymbolicError {
    pub fn into_rich<'src>(self, span: Span) -> Rich<'src, String, Span> {
        Rich::custom(span, format!("{self}"))
    }
}

pub struct SymbolResolver {
    scopes: Vec<BTreeMap<String, Symbol>>,
    next_symbol: u32,
}

impl SymbolResolver {
    pub fn new() -> SymbolResolver {
        SymbolResolver {
            scopes: vec![BTreeMap::new()],
            next_symbol: 0,
        }
    }
}

impl SymbolResolver {
    fn next_symbol(&mut self, original: String) -> Symbol {
        self.next_symbol += 1;

        Symbol {
            id: self.next_symbol,
            original,
        }
    }
}

impl AstTransform for SymbolResolver {
    type Error = Spanned<SymbolicError>;
    type From = ParsedMetadata;
    type To = SymbolMetadata;

    fn transform_definition(
        &mut self,
        definition: Definition<Self::From>,
        meta: ParsedMetadata,
    ) -> Results<(Definition<Self::To>, Self::To), Self::Error> {
        let symbol = self.next_symbol(definition.name.clone());

        self.scopes
            .last_mut()
            .unwrap()
            .insert(definition.name, symbol.clone());

        self.transform(definition.rhs).map(|rhs| {
            (
                Definition {
                    name: Some(symbol),
                    rhs,
                },
                SymbolMetadata { span: meta.span },
            )
        })
    }

    fn transform_return(
        &mut self,
        rtn: ast::Return<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(ast::Return<Self::To>, Self::To), Self::Error> {
        self.transform(rtn.expression).map(|expression| {
            (
                ast::Return { expression },
                SymbolMetadata { span: meta.span },
            )
        })
    }

    fn transform_match(
        &mut self,
        mtch: ast::Match<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(ast::Match<Self::To>, Self::To), Self::Error> {
        self.transform(mtch.value)
            .zip(
                mtch.conds
                    .into_iter()
                    .map(|(cond, body)| self.transform(cond).zip(self.transform(body)))
                    .collect(),
            )
            .map(|(value, conds)| {
                (
                    ast::Match { value, conds },
                    SymbolMetadata { span: meta.span },
                )
            })
    }

    fn transform_ident(
        &mut self,
        ident: Ident<Self::From>,
        meta: ParsedMetadata,
    ) -> Results<(Ident<Self::To>, Self::To), Self::Error> {
        match self
            .scopes
            .iter()
            .rev()
            .filter_map(|scope| scope.get(&ident.value))
            .next()
        {
            Some(value) => Results::ok((
                Ident {
                    value: Some(value.clone()),
                },
                SymbolMetadata { span: meta.span },
            )),
            None => Results::with_error(
                (Ident { value: None }, SymbolMetadata { span: meta.span }),
                SymbolicError::Undefined { name: ident.value }.with_span(meta.span),
            ),
        }
    }

    fn transform_number(
        &mut self,
        number: NumberLiteral,
        meta: ParsedMetadata,
    ) -> Results<(NumberLiteral, Self::To), Self::Error> {
        Results::ok((number, SymbolMetadata { span: meta.span }))
    }

    fn transform_string(
        &mut self,
        string: StringLiteral,
        meta: ParsedMetadata,
    ) -> Results<(StringLiteral, Self::To), Self::Error> {
        Results::ok((string, SymbolMetadata { span: meta.span }))
    }

    fn transform_boolean(
        &mut self,
        boolean: BooleanLiteral,
        meta: ParsedMetadata,
    ) -> Results<(BooleanLiteral, Self::To), Self::Error> {
        Results::ok((boolean, SymbolMetadata { span: meta.span }))
    }

    fn transform_array(
        &mut self,
        array: Array<Self::From>,
        meta: ParsedMetadata,
    ) -> Results<(Array<Self::To>, Self::To), Self::Error> {
        array
            .items
            .into_iter()
            .map(|e| self.transform(e))
            .collect::<Results<Vec<_>, _>>()
            .map(|items| (Array { items }, SymbolMetadata { span: meta.span }))
    }

    fn transform_object(
        &mut self,
        object: Object<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(Object<Self::To>, Self::To), Self::Error> {
        object
            .fields
            .into_iter()
            .map(|(name, e)| self.transform(e).map(|v| (name, v)))
            .collect::<Results<BTreeMap<_, _>, _>>()
            .map(|fields| (Object { fields }, SymbolMetadata { span: meta.span }))
    }

    fn transform_lambda(
        &mut self,
        lambda: Lambda<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(Lambda<Self::To>, Self::To), Self::Error> {
        self.scopes.push(Default::default());

        let params = lambda
            .params
            .into_iter()
            .map(|(name, ty)| {
                let symbol = self.next_symbol(name.clone());
                self.scopes.last_mut().unwrap().insert(name, symbol.clone());
                (Some(symbol), ty)
            })
            .collect();

        let body = lambda
            .body
            .into_iter()
            .map(|e| self.transform(e))
            .collect::<Results<Vec<_>, _>>();

        self.scopes.pop();

        body.map(|body| (Lambda { params, body }, SymbolMetadata { span: meta.span }))
    }

    fn transform_unary_op(
        &mut self,
        unary_operation: UnaryOperation<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(UnaryOperation<Self::To>, Self::To), Self::Error> {
        self.transform(unary_operation.arg).map(|arg| {
            (
                UnaryOperation {
                    arg,
                    op: unary_operation.op,
                },
                SymbolMetadata { span: meta.span },
            )
        })
    }

    fn transform_binary_op(
        &mut self,
        binary_operation: BinaryOperation<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(BinaryOperation<Self::To>, Self::To), Self::Error> {
        self.transform(binary_operation.lhs)
            .zip(self.transform(binary_operation.rhs))
            .map(|(lhs, rhs)| {
                (
                    BinaryOperation {
                        lhs,
                        rhs,
                        op: binary_operation.op,
                    },
                    SymbolMetadata { span: meta.span },
                )
            })
    }

    fn transform_call(
        &mut self,
        call: Call<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(Call<Self::To>, Self::To), Self::Error> {
        self.transform(call.expression)
            .zip(
                call.args
                    .into_iter()
                    .map(|(name, e)| self.transform(e).map(|expr| (name, expr)))
                    .collect(),
            )
            .map(|(expression, args)| {
                (
                    Call { expression, args },
                    SymbolMetadata { span: meta.span },
                )
            })
    }

    fn transform_object_access(
        &mut self,
        object_access: ObjectAccess<ParsedMetadata>,
        meta: ParsedMetadata,
    ) -> Results<(ObjectAccess<Self::To>, Self::To), Self::Error> {
        self.transform(object_access.expression).map(|expression| {
            (
                ObjectAccess {
                    expression,
                    field: object_access.field,
                },
                SymbolMetadata { span: meta.span },
            )
        })
    }
}
