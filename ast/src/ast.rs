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

pub trait Ast: Debug + Clone + PartialEq {
    type Node<T>;

    type Expression: Clone + Debug + PartialEq;
    type Pattern: Clone + Debug + PartialEq;
    type Type: Clone + Debug + PartialEq;
    type Statement: Clone + Debug + PartialEq;

    type Ident: Clone + Debug + PartialEq + PartialEq + Eq + PartialOrd + Ord;
    type U: Clone + Debug + PartialEq;
    type B: Clone + Debug + PartialEq;

    type Meta: Clone + Debug + PartialEq;
}

impl<T> Ast for &T
where
    T: Ast + Sized,
{
    type Node<U> = T::Node<U>;
    type Expression = T::Expression;
    type Pattern = T::Pattern;
    type Type = T::Type;
    type Statement = T::Statement;
    type Ident = T::Ident;
    type U = T::U;
    type B = T::B;
    type Meta = T::Meta;
}

type TransformResult<S, A, E> = Results<AstNode<S, A>, E>;
pub type Programs<A> = Vec<AstNode<Program<A>, A>>;

type TransformProgramsResult<A, E> = Results<Programs<A>, E>;

pub trait Transform<T, V>: AstTransform {
    fn transform(
        &mut self,
        from: AstNode<T, Self::From>,
    ) -> Results<AstNode<V, Self::To>, Self::Error>;
}

impl<T, V, A: AstTransform> Transform<T, V> for A
where
    V: From<T>,
    <Self::To as Ast>::Meta: From<<Self::From as Ast>::Meta>,
{
    fn transform(
        &mut self,
        from: AstNode<T, Self::From>,
    ) -> Results<AstNode<V, Self::To>, A::Error> {
        Results::ok(AstNode {
            inner: from.inner.into(),
            meta: <Self::To as Ast>::Meta::from(from.meta),
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct AstNode<T, A: Ast> {
    pub inner: T,
    pub meta: A::Meta,
}

impl<T, A: Ast> AstNode<T, A> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AstNode<U, A> {
        AstNode {
            inner: f(self.inner),
            meta: self.meta,
        }
    }

    pub fn try_map<U, E>(self, f: impl FnOnce(T) -> Results<U, E>) -> Results<AstNode<U, A>, E> {
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
}

impl<A: AstTransform> Transform<Expression<A::From>, Expression<A::To>> for A
where
    Self: Transform<Match<<Self as AstTransform>::From>, Match<<Self as AstTransform>::To>>
        + Transform<Ident<<Self as AstTransform>::From>, Ident<<Self as AstTransform>::To>>
        + Transform<Primative, Primative>
        + Transform<Array<<Self as AstTransform>::From>, Array<<Self as AstTransform>::To>>
        + Transform<Object<<Self as AstTransform>::From>, Object<<Self as AstTransform>::To>>
        + Transform<Lambda<<Self as AstTransform>::From>, Lambda<<Self as AstTransform>::To>>
        + Transform<Node, Node>
        + Transform<
            UnaryOperation<<Self as AstTransform>::From>,
            UnaryOperation<<Self as AstTransform>::To>,
        > + Transform<
            BinaryOperation<<Self as AstTransform>::From>,
            BinaryOperation<<Self as AstTransform>::To>,
        > + Transform<Call<<Self as AstTransform>::From>, Call<<Self as AstTransform>::To>>
        + Transform<
            ObjectAccess<<Self as AstTransform>::From>,
            ObjectAccess<<Self as AstTransform>::To>,
        > + Transform<Block<<Self as AstTransform>::From>, Block<<Self as AstTransform>::To>>,
{
    fn transform(
        &mut self,
        from: AstNode<Expression<A::From>, Self::From>,
    ) -> Results<AstNode<Expression<A::To>, Self::To>, Self::Error> {
        todo!()
    }
}

pub trait AstTraverse: AstTransform {
    fn transform_all(
        &mut self,
        programs: Programs<Self::From>,
    ) -> TransformProgramsResult<Self::To, Self::Error>
    where
        Self: AstTransform
            + Transform<Program<<Self as AstTransform>::From>, Program<<Self as AstTransform>::To>>,
    {
        programs
            .into_iter()
            .map(|program| self.transform(program))
            .collect()
    }

    fn dispatch<T, V, F>(
        &mut self,
        meta: <Self::From as Ast>::Meta,
        inner: T,
        wrap: impl FnOnce(V) -> F,
    ) -> TransformResult<F, Self::To, Self::Error>
    where
        Self: Transform<T, V>,
    {
        self.transform(AstNode { meta, inner }).map(|node| AstNode {
            meta: node.meta,
            inner: wrap(node.inner),
        })
    }

    fn transform_primative(
        &mut self,
        AstNode { inner, meta }: AstNode<Primative, Self::From>,
    ) -> TransformResult<Primative, Self::To, Self::Error>
    where
        Self: Transform<bool, bool> + Transform<f64, f64> + Transform<String, String>,
    {
        match inner {
            Primative::Boolean(x) => self.dispatch(meta, x, Primative::Boolean),
            Primative::Number(x) => self.dispatch(meta, x, Primative::Number),
            Primative::String(x) => self.dispatch(meta, x, Primative::String),
        }
    }

    fn transform_expression(
        &mut self,
        AstNode { inner, meta }: AstNode<Expression<Self::From>, Self::From>,
    ) -> TransformResult<Expression<Self::To>, Self::To, Self::Error>
    where
        Self: Transform<Match<<Self as AstTransform>::From>, Match<<Self as AstTransform>::To>>
            + Transform<Ident<<Self as AstTransform>::From>, Ident<<Self as AstTransform>::To>>
            + Transform<Primative, Primative>
            + Transform<Array<<Self as AstTransform>::From>, Array<<Self as AstTransform>::To>>
            + Transform<Object<<Self as AstTransform>::From>, Object<<Self as AstTransform>::To>>
            + Transform<Lambda<<Self as AstTransform>::From>, Lambda<<Self as AstTransform>::To>>
            + Transform<Node, Node>
            + Transform<
                UnaryOperation<<Self as AstTransform>::From>,
                UnaryOperation<<Self as AstTransform>::To>,
            > + Transform<
                BinaryOperation<<Self as AstTransform>::From>,
                BinaryOperation<<Self as AstTransform>::To>,
            > + Transform<Call<<Self as AstTransform>::From>, Call<<Self as AstTransform>::To>>
            + Transform<
                ObjectAccess<<Self as AstTransform>::From>,
                ObjectAccess<<Self as AstTransform>::To>,
            > + Transform<Block<<Self as AstTransform>::From>, Block<<Self as AstTransform>::To>>,
    {
        match inner {
            Expression::Match(x) => self.dispatch(meta, x, Expression::Match),
            Expression::Ident(x) => self.dispatch(meta, x, Expression::Ident),
            Expression::Primative(x) => self.dispatch(meta, x, Expression::Primative),
            Expression::Array(x) => self.dispatch(meta, x, Expression::Array),
            Expression::Object(x) => self.dispatch(meta, x, Expression::Object),
            Expression::Lambda(x) => self.dispatch(meta, x, Expression::Lambda),
            Expression::Node(x) => self.dispatch(meta, x, Expression::Node),
            Expression::UnaryOperation(x) => self.dispatch(meta, x, Expression::UnaryOperation),
            Expression::BinaryOperation(x) => self.dispatch(meta, x, Expression::BinaryOperation),
            Expression::Call(x) => self.dispatch(meta, x, Expression::Call),
            Expression::ObjectAccess(x) => self.dispatch(meta, x, Expression::ObjectAccess),
            Expression::Block(x) => self.dispatch(meta, x, Expression::Block),
        }
    }

    fn transform_pattern(
        &mut self,
        AstNode { inner, meta }: AstNode<Pattern<Self::From>, Self::From>,
    ) -> TransformResult<Pattern<Self::To>, Self::To, Self::Error>
    where
        Self: Transform<
                ObjectDestructure<<Self as AstTransform>::From>,
                ObjectDestructure<<Self as AstTransform>::To>,
            > + Transform<
                ArrayDestructure<<Self as AstTransform>::From>,
                ArrayDestructure<<Self as AstTransform>::To>,
            > + Transform<EnumDestructure, EnumDestructure>
            + Transform<Ident<<Self as AstTransform>::From>, Ident<<Self as AstTransform>::To>>
            + Transform<Primative, Primative>,
    {
        match inner {
            Pattern::Object(x) => self.dispatch(meta, x, Pattern::Object),
            Pattern::Array(x) => self.dispatch(meta, x, Pattern::Array),
            Pattern::Enum(x) => self.dispatch(meta, x, Pattern::Enum),
            Pattern::Binding(x) => self.dispatch(meta, x, Pattern::Binding),
            Pattern::Primative(x) => self.dispatch(meta, x, Pattern::Primative),
        }
    }

    fn transform_statement(
        &mut self,
        AstNode { inner, meta }: AstNode<Statement<Self::From>, Self::From>,
    ) -> TransformResult<Statement<Self::To>, Self::To, Self::Error>
    where
        Self: Transform<
                Definition<<Self as AstTransform>::From>,
                Definition<<Self as AstTransform>::To>,
            > + Transform<
                TypeDefinition<<Self as AstTransform>::From>,
                TypeDefinition<<Self as AstTransform>::To>,
            > + Transform<Return<<Self as AstTransform>::From>, Return<<Self as AstTransform>::To>>
            + Transform<
                Expression<<Self as AstTransform>::From>,
                Expression<<Self as AstTransform>::To>,
            >,
    {
        match inner {
            Statement::Definition(x) => self.dispatch(meta, x, Statement::Definition),
            Statement::TypeDefinition(x) => self.dispatch(meta, x, Statement::TypeDefinition),
            Statement::Return(x) => self.dispatch(meta, x, Statement::Return),
            Statement::Expression(x) => self.dispatch(meta, x, Statement::Expression),
        }
    }

    fn transform_program(
        &mut self,
        AstNode { inner, meta }: AstNode<Program<Self::From>, Self::From>,
    ) -> TransformResult<Program<Self::To>, Self::To, Self::Error>
    where
        Self: Transform<Import<<Self as AstTransform>::From>, Import<<Self as AstTransform>::To>>
            + Transform<
                Statement<<Self as AstTransform>::From>,
                Statement<<Self as AstTransform>::To>,
            >,
    {
        match inner {
            Program::Import(x) => self.dispatch(meta, x, Program::Import),
            Program::Statement(x) => self.dispatch(meta, x, Program::Statement),
        }
    }

    fn transform_type(
        &mut self,
        AstNode { inner, meta }: AstNode<Type<Self::From>, Self::From>,
    ) -> TransformResult<Type<Self::To>, Self::To, Self::Error>
    where
        Self: Transform<NodeType, NodeType>
            + Transform<StringType, StringType>
            + Transform<NumberType, NumberType>
            + Transform<BooleanType, BooleanType>
            + Transform<
                ObjectType<<Self as AstTransform>::From>,
                ObjectType<<Self as AstTransform>::To>,
            > + Transform<
                LambdaType<<Self as AstTransform>::From>,
                LambdaType<<Self as AstTransform>::To>,
            > + Transform<
                ArrayType<<Self as AstTransform>::From>,
                ArrayType<<Self as AstTransform>::To>,
            > + Transform<
                OptionalType<<Self as AstTransform>::From>,
                OptionalType<<Self as AstTransform>::To>,
            > + Transform<
                UnionType<<Self as AstTransform>::From>,
                UnionType<<Self as AstTransform>::To>,
            > + Transform<NoneType, NoneType>
            + Transform<
                TupleType<<Self as AstTransform>::From>,
                TupleType<<Self as AstTransform>::To>,
            > + Transform<NeverType, NeverType>,
    {
        match inner {
            Type::Node(x) => self.dispatch(meta, x, Type::Node),
            Type::String(x) => self.dispatch(meta, x, Type::String),
            Type::Number(x) => self.dispatch(meta, x, Type::Number),
            Type::Boolean(x) => self.dispatch(meta, x, Type::Boolean),
            Type::Object(x) => self.dispatch(meta, x, Type::Object),
            Type::Lambda(x) => self.dispatch(meta, x, Type::Lambda),
            Type::Array(x) => self.dispatch(meta, x, Type::Array),
            Type::Optional(x) => self.dispatch(meta, x, Type::Optional),
            Type::Union(x) => self.dispatch(meta, x, Type::Union),
            Type::None(x) => self.dispatch(meta, x, Type::None),
            Type::Tuple(x) => self.dispatch(meta, x, Type::Tuple),
            Type::Never(x) => self.dispatch(meta, x, Type::Never),
        }
    }
}

impl<T: AstTransform> AstTraverse for T {}
