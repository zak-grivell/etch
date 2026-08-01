use crate::{
    Array, ArrayType, BooleanType, Definition, LambdaType, NeverType, NodeType, NoneType,
    NumberType, ObjectType, OptionalType, Return, StringType, TupleType, Type, UnionType,
    expression::*,
    pattern::{ArrayDestructure, EnumDestructure, ObjectDestructure, Pattern},
    primative::Primative,
    program::{Import, Program},
    result::Results,
    statement::{Statement, TypeDefinition},
};
use std::fmt::Debug;

pub trait Ast: Sized + Debug + Clone + PartialEq {
    type Node<T>;

    type Expression: Clone + Debug + PartialEq;
    type Pattern: Clone + Debug + PartialEq;
    type Type: Clone + Debug + PartialEq;
    type Statement: Clone + Debug + PartialEq;

    type Ident: Clone + Debug + PartialEq + PartialEq + Eq + PartialOrd + Ord;
    type U: Clone + Debug + PartialEq;
    type B: Clone + Debug + PartialEq;
}

type TransformResult<S, T, E> = Results<AstNode<S, T>, E>;

#[derive(Debug, PartialEq, Clone)]
pub struct AstNode<T, M: Ast> {
    pub inner: T,
    pub meta: M,
}

impl<T, M: Ast> AstNode<T, M> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AstNode<U, M> {
        AstNode {
            inner: f(self.inner),
            meta: self.meta,
        }
    }

    pub fn try_map<U, E>(self, f: impl FnOnce(T) -> Results<U, E>) -> Results<AstNode<U, M>, E> {
        f(self.inner).map(|inner| AstNode {
            meta: self.meta,
            inner,
        })
    }
}

pub trait AstTransform {
    type Error;
    type From: Ast;
    type To: Ast;

    fn dispatch<I, O, R>(
        &mut self,
        meta: Self::From,
        inner: I,
        f: impl FnOnce(&mut Self, AstNode<I, Self::From>) -> TransformResult<O, Self::To, Self::Error>,
    ) -> TransformResult<R, Self::To, Self::Error>
    where
        R: From<O>,
    {
        f(self, AstNode { inner, meta }).map(|node| AstNode {
            inner: R::from(node.inner),
            meta: node.meta,
        })
    }

    fn transform_definition(
        &mut self,
        definition: AstNode<Definition<Self::From>, Self::From>,
    ) -> TransformResult<Definition<Self::To>, Self::To, Self::Error>;

    fn transform_type_definition(
        &mut self,
        type_definition: AstNode<TypeDefinition<Self::From>, Self::From>,
    ) -> TransformResult<TypeDefinition<Self::To>, Self::To, Self::Error>;

    fn transform_return(
        &mut self,
        rtn: AstNode<Return<Self::From>, Self::From>,
    ) -> TransformResult<Return<Self::To>, Self::To, Self::Error>;

    fn transform_match(
        &mut self,
        mtch: AstNode<Match<Self::From>, Self::From>,
    ) -> TransformResult<Match<Self::To>, Self::To, Self::Error>;

    fn transform_ident(
        &mut self,
        ident: AstNode<Ident<Self::From>, Self::From>,
    ) -> TransformResult<Ident<Self::To>, Self::To, Self::Error>;

    fn transform_array(
        &mut self,
        array: AstNode<Array<Self::From>, Self::From>,
    ) -> TransformResult<Array<Self::To>, Self::To, Self::Error>;

    fn transform_object(
        &mut self,
        object: AstNode<Object<Self::From>, Self::From>,
    ) -> TransformResult<Object<Self::To>, Self::To, Self::Error>;

    fn transform_lambda(
        &mut self,
        lambda: AstNode<Lambda<Self::From>, Self::From>,
    ) -> TransformResult<Lambda<Self::To>, Self::To, Self::Error>;

    fn transform_unary_op(
        &mut self,
        unary_operation: AstNode<UnaryOperation<Self::From>, Self::From>,
    ) -> TransformResult<UnaryOperation<Self::To>, Self::To, Self::Error>;

    fn transform_binary_op(
        &mut self,
        binary_operation: AstNode<BinaryOperation<Self::From>, Self::From>,
    ) -> TransformResult<BinaryOperation<Self::To>, Self::To, Self::Error>;

    fn transform_call(
        &mut self,
        call: AstNode<Call<Self::From>, Self::From>,
    ) -> TransformResult<Call<Self::To>, Self::To, Self::Error>;

    fn transform_object_access(
        &mut self,
        object_access: AstNode<ObjectAccess<Self::From>, Self::From>,
    ) -> TransformResult<ObjectAccess<Self::To>, Self::To, Self::Error>;

    fn transform_node(
        &mut self,
        match_definition: AstNode<Node, Self::From>,
    ) -> TransformResult<Node, Self::To, Self::Error>;

    fn transform_object_pattern(
        &mut self,
        object_pattern: AstNode<ObjectDestructure<Self::From>, Self::From>,
    ) -> TransformResult<ObjectDestructure<Self::To>, Self::To, Self::Error>;

    fn transform_array_pattern(
        &mut self,
        array_pattern: AstNode<ArrayDestructure<Self::From>, Self::From>,
    ) -> TransformResult<ArrayDestructure<Self::To>, Self::To, Self::Error>;

    fn transform_enum_pattern(
        &mut self,
        enum_pattern: AstNode<EnumDestructure, Self::From>,
    ) -> TransformResult<EnumDestructure, Self::To, Self::Error>;

    fn transform_import(
        &mut self,
        import: AstNode<Import<Self::From>, Self::From>,
    ) -> TransformResult<Import<Self::To>, Self::To, Self::Error>;

    fn transform_type_node(
        &mut self,
        node: AstNode<NodeType, Self::From>,
    ) -> TransformResult<NodeType, Self::To, Self::Error>;

    fn transform_type_number(
        &mut self,
        node: AstNode<NumberType, Self::From>,
    ) -> TransformResult<NumberType, Self::To, Self::Error>;

    fn transform_type_string(
        &mut self,
        node: AstNode<StringType, Self::From>,
    ) -> TransformResult<StringType, Self::To, Self::Error>;

    fn transform_type_boolean(
        &mut self,
        node: AstNode<BooleanType, Self::From>,
    ) -> TransformResult<BooleanType, Self::To, Self::Error>;

    fn transform_type_none(
        &mut self,
        node: AstNode<NoneType, Self::From>,
    ) -> TransformResult<NoneType, Self::To, Self::Error>;

    fn transform_type_never(
        &mut self,
        node: AstNode<NeverType, Self::From>,
    ) -> TransformResult<NeverType, Self::To, Self::Error>;

    fn transform_type_object(
        &mut self,
        node: AstNode<ObjectType<Self::From>, Self::From>,
    ) -> TransformResult<ObjectType<Self::To>, Self::To, Self::Error>;

    fn transform_type_array(
        &mut self,
        node: AstNode<ArrayType<Self::From>, Self::From>,
    ) -> TransformResult<ArrayType<Self::To>, Self::To, Self::Error>;

    fn transform_type_optional(
        &mut self,
        node: AstNode<OptionalType<Self::From>, Self::From>,
    ) -> TransformResult<OptionalType<Self::To>, Self::To, Self::Error>;

    fn transform_type_lambda(
        &mut self,
        node: AstNode<LambdaType<Self::From>, Self::From>,
    ) -> TransformResult<LambdaType<Self::To>, Self::To, Self::Error>;

    fn transform_type_union(
        &mut self,
        node: AstNode<UnionType<Self::From>, Self::From>,
    ) -> TransformResult<UnionType<Self::To>, Self::To, Self::Error>;

    fn transform_type_tuple(
        &mut self,
        node: AstNode<TupleType<Self::From>, Self::From>,
    ) -> TransformResult<TupleType<Self::To>, Self::To, Self::Error>;

    fn transform_string(
        &mut self,
        string: AstNode<String, Self::From>,
    ) -> TransformResult<String, Self::To, Self::Error>;

    fn transform_boolean(
        &mut self,
        boolean: AstNode<bool, Self::From>,
    ) -> TransformResult<bool, Self::To, Self::Error>;

    fn transform_number(
        &mut self,
        number: AstNode<f64, Self::From>,
    ) -> TransformResult<f64, Self::To, Self::Error>;

    fn transform_primative(
        &mut self,
        AstNode { inner, meta }: AstNode<Primative, Self::From>,
    ) -> TransformResult<Primative, Self::To, Self::Error> {
        match inner {
            Primative::Boolean(x) => self.dispatch(meta, x, Self::transform_boolean),
            Primative::Number(x) => self.dispatch(meta, x, Self::transform_number),
            Primative::String(x) => self.dispatch(meta, x, Self::transform_string),
        }
    }

    fn transform_block(
        &mut self,
        block: AstNode<Block<Self::From>, Self::From>,
    ) -> TransformResult<Block<Self::To>, Self::To, Self::Error>;

    fn transform_expression(
        &mut self,
        AstNode { inner, meta }: AstNode<Expression<Self::From>, Self::From>,
    ) -> TransformResult<Expression<Self::To>, Self::To, Self::Error> {
        match inner {
            Expression::Match(x) => self.dispatch(meta, x, Self::transform_match),
            Expression::Ident(x) => self.dispatch(meta, x, Self::transform_ident),
            Expression::Primative(x) => self.dispatch(meta, x, Self::transform_primative),
            Expression::Array(x) => self.dispatch(meta, x, Self::transform_array),
            Expression::Object(x) => self.dispatch(meta, x, Self::transform_object),
            Expression::Lambda(x) => self.dispatch(meta, x, Self::transform_lambda),
            Expression::Node(x) => self.dispatch(meta, x, Self::transform_node),
            Expression::UnaryOperation(x) => self.dispatch(meta, x, Self::transform_unary_op),
            Expression::BinaryOperation(x) => self.dispatch(meta, x, Self::transform_binary_op),
            Expression::Call(x) => self.dispatch(meta, x, Self::transform_call),
            Expression::ObjectAccess(x) => self.dispatch(meta, x, Self::transform_object_access),
            Expression::Block(x) => self.dispatch(meta, x, Self::transform_block),
        }
    }

    fn transform_pattern(
        &mut self,
        AstNode { inner, meta }: AstNode<Pattern<Self::From>, Self::From>,
    ) -> TransformResult<Pattern<Self::To>, Self::To, Self::Error> {
        match inner {
            Pattern::Object(x) => self.dispatch(meta, x, Self::transform_object_pattern),
            Pattern::Array(x) => self.dispatch(meta, x, Self::transform_array_pattern),
            Pattern::Enum(x) => self.dispatch(meta, x, Self::transform_enum_pattern),
            Pattern::Binding(x) => self.dispatch(meta, x, Self::transform_ident),
            Pattern::Primative(x) => self.dispatch(meta, x, Self::transform_primative),
        }
    }

    fn transform_statement(
        &mut self,
        AstNode { inner, meta }: AstNode<Statement<Self::From>, Self::From>,
    ) -> TransformResult<Statement<Self::To>, Self::To, Self::Error> {
        match inner {
            Statement::Definition(x) => self.dispatch(meta, x, Self::transform_definition),
            Statement::TypeDefinition(x) => self.dispatch(meta, x, Self::transform_type_definition),
            Statement::Return(x) => self.dispatch(meta, x, Self::transform_return),
            Statement::Expression(x) => self.dispatch(meta, x, Self::transform_expression),
        }
    }

    fn transform_program(
        &mut self,
        AstNode { inner, meta }: AstNode<Program<Self::From>, Self::From>,
    ) -> TransformResult<Program<Self::To>, Self::To, Self::Error>
    where
        Self::From: Ast<Statement = Statement<Self::From>>,
        Self::To: Ast<Statement = Statement<Self::To>>,
    {
        match inner {
            Program::Import(x) => self.dispatch(meta, x, Self::transform_import),
            Program::Statement(x) => self.dispatch(meta, x, Self::transform_statement),
        }
    }

    fn transform_type(
        &mut self,
        ty: AstNode<Type<Self::From>, Self::From>,
    ) -> TransformResult<Type<Self::To>, Self::To, Self::Error> {
        let AstNode { inner, meta } = ty;
        match inner {
            Type::Node(x) => self.dispatch(meta, x, Self::transform_type_node),
            Type::String(x) => self.dispatch(meta, x, Self::transform_type_string),
            Type::Number(x) => self.dispatch(meta, x, Self::transform_type_number),
            Type::Boolean(x) => self.dispatch(meta, x, Self::transform_type_boolean),
            Type::Object(x) => self.dispatch(meta, x, Self::transform_type_object),
            Type::Lambda(x) => self.dispatch(meta, x, Self::transform_type_lambda),
            Type::Array(x) => self.dispatch(meta, x, Self::transform_type_array),
            Type::Union(x) => self.dispatch(meta, x, Self::transform_type_union),
            Type::None(x) => self.dispatch(meta, x, Self::transform_type_none),
            Type::Tuple(x) => self.dispatch(meta, x, Self::transform_type_tuple),
            Type::Never(x) => self.dispatch(meta, x, Self::transform_type_never),
        }
    }
}
