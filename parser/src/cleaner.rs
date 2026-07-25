use std::{collections::BTreeMap};

use ast::{
    Array, Ast, AstNode, AstTransform, BinaryOperation, BinaryOperator, BooleanLiteral, Call,
    Definition, Expression, Ident, Lambda, NumberLiteral, Object, ObjectAccess, PartialType,
    Results, Span, StringLiteral, StrongType, Symbol, UnaryOperation, UnaryOperator,
};
use chumsky::span::{SpanWrap, Spanned};

use crate::parser::ParsedExpression;
use crate::semantic::{PartiallyTypedExpression, TypedBinaryOperator, TypedUnaryOperator};

#[derive(Debug, Clone, PartialEq)]
struct StrippedExpression {
    expression: Expression<StrippedExpression>,
    t: StrongType,
}

impl Ast for StrippedExpression {
    type E = AstNode<Self>;
    type I = Symbol;
    type T = StrongType;

    type B = TypedBinaryOperator;
    type U = TypedUnaryOperator;

    fn expression(self) -> Expression<Self> {
        self.expression
    }
}

struct ExpressionStripper;

impl AstTransform for ExpressionStripper {
    type Error = ();
    type From = PartiallyTypedExpression;
    type To = StrippedExpression;

    fn transform_definition(
        &mut self,
        definition: Definition<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(definition.rhs)
            .map(|rhs| StrippedExpression {
                expression: Expression::new_definition(symbol, rhs),
                t: definition.
            })
    }

    fn transform_return(
        &mut self,
        rtn: ast::Return<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(rtn.expression)
            .map(|expression| SymbolicExpression {
                expression: Expression::new_return(expression),
            })
    }

    fn transform_match(
        &mut self,
        mtch: ast::Match<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(mtch.value)
            .zip(
                mtch.conds
                    .into_iter()
                    .map(|(cond, body)| self.transform(cond).zip(self.transform(body)))
                    .collect(),
            )
            .map(|(value, conds)| SymbolicExpression {
                expression: Expression::new_match(value, conds),
            })
    }

    fn transform_ident(
        &mut self,
        ident: Ident<Self::From>,

        span: Span,
    ) -> Results<Self::To, Self::Error> {
        match self
            .scopes
            .iter()
            .rev()
            .filter_map(|scope| scope.get(&ident.value))
            .next()
        {
            Some(value) => Results::ok(Some(value.clone())),
            None => Results::with_error(
                None,
                SymbolicError::Undefined { name: ident.value }.with_span(span),
            ),
        }
        .map(|symbol| SymbolicExpression {
            expression: Expression::new_ident(symbol),
        })
    }

    fn transform_number(
        &mut self,
        number: NumberLiteral,

        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(number).map(|number| SymbolicExpression {
            expression: Expression::Number(number),
        })
    }

    fn transform_string(
        &mut self,
        string: StringLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(string).map(|string| SymbolicExpression {
            expression: Expression::String(string),
        })
    }

    fn transform_boolean(
        &mut self,
        boolean: BooleanLiteral,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        Results::ok(boolean).map(|boolean| SymbolicExpression {
            expression: Expression::Boolean(boolean),
        })
    }

    fn transform_array(
        &mut self,
        array: Array<Self::From>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        array
            .items
            .into_iter()
            .map(|e| self.transform(e))
            .collect::<Results<Vec<_>, _>>()
            .map(|array| SymbolicExpression {
                expression: Expression::new_array(array),
            })
    }

    fn transform_object(
        &mut self,
        object: Object<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        object
            .fields
            .into_iter()
            .map(|(name, e)| self.transform(e).map(|v| (name, v)))
            .collect::<Results<BTreeMap<_, _>, _>>()
            .map(|fields| SymbolicExpression {
                expression: Expression::new_object(fields),
            })
    }

    fn transform_lambda(
        &mut self,
        lambda: Lambda<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
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

        body.map(|body| SymbolicExpression {
            expression: Expression::new_lambda(params, body),
        })
    }

    fn transform_unary_op(
        &mut self,
        unary_operation: UnaryOperation<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(unary_operation.arg)
            .map(|arg| SymbolicExpression {
                expression: Expression::new_unary_operation(arg, unary_operation.op),
            })
    }

    fn transform_binary_op(
        &mut self,
        binary_operation: BinaryOperation<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(binary_operation.lhs)
            .zip(self.transform(binary_operation.rhs))
            .map(|(lhs, rhs)| SymbolicExpression {
                expression: Expression::new_binary_operation(lhs, rhs, binary_operation.op),
            })
    }

    fn transform_call(
        &mut self,
        call: Call<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(call.expression)
            .zip(
                call.args
                    .into_iter()
                    .map(|(name, e)| {
                        match self
                            .scopes
                            .iter()
                            .rev()
                            .filter_map(|scope| scope.get(&name))
                            .next()
                        {
                            Some(value) => Results::ok(Some(value.clone())),
                            None => Results::with_error(
                                None,
                                SymbolicError::Undefined { name }.with_span(span),
                            ),
                        }
                        .zip(self.transform(e))
                    })
                    .collect(),
            )
            .map(|(expression, args)| SymbolicExpression {
                expression: Expression::new_call(expression, args),
            })
    }

    fn transform_object_access(
        &mut self,
        object_access: ObjectAccess<ParsedExpression>,
        span: Span,
    ) -> Results<Self::To, Self::Error> {
        self.transform(object_access.expression)
            .map(|expression| SymbolicExpression {
                expression: Expression::new_object_access(expression, object_access.field),
            })
    }
}
