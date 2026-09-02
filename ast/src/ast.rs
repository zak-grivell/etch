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
type FromOf<T> = <T as AstTransform>::From;
type ToOf<T> = <T as AstTransform>::To;

pub trait Transform<T, V>: AstTransform {
    fn transform(
        &mut self,
        from: AstNode<T, Self::From>,
    ) -> Results<AstNode<V, Self::To>, Self::Error>;
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

macro_rules! transform_method {
    ($name:ident, $input:ty, $output:ty) => {
        fn $name(
            &mut self,
            node: AstNode<$input, Self::From>,
        ) -> TransformResult<$output, Self::To, Self::Error>
        where
            Self: Transform<$input, $output>,
        {
            self.transform(node)
        }
    };
}

pub trait AstTraverse: AstTransform {
    fn transform_all(
        &mut self,
        programs: Programs<Self::From>,
    ) -> TransformProgramsResult<Self::To, Self::Error>
    where
        Self: Transform<Program<<Self as AstTransform>::From>, Program<<Self as AstTransform>::To>>,
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

    transform_method!(
        transform_definition,
        Definition<FromOf<Self>>,
        Definition<ToOf<Self>>
    );
    transform_method!(
        transform_type_definition,
        TypeDefinition<FromOf<Self>>,
        TypeDefinition<ToOf<Self>>
    );
    transform_method!(transform_return, Return<FromOf<Self>>, Return<ToOf<Self>>);
    transform_method!(transform_match, Match<FromOf<Self>>, Match<ToOf<Self>>);
    transform_method!(transform_ident, Ident<FromOf<Self>>, Ident<ToOf<Self>>);
    transform_method!(transform_array, Array<FromOf<Self>>, Array<ToOf<Self>>);
    transform_method!(transform_object, Object<FromOf<Self>>, Object<ToOf<Self>>);
    transform_method!(transform_lambda, Lambda<FromOf<Self>>, Lambda<ToOf<Self>>);
    transform_method!(
        transform_unary_op,
        UnaryOperation<FromOf<Self>>,
        UnaryOperation<ToOf<Self>>
    );
    transform_method!(
        transform_binary_op,
        BinaryOperation<FromOf<Self>>,
        BinaryOperation<ToOf<Self>>
    );
    transform_method!(transform_call, Call<FromOf<Self>>, Call<ToOf<Self>>);
    transform_method!(
        transform_object_access,
        ObjectAccess<FromOf<Self>>,
        ObjectAccess<ToOf<Self>>
    );
    transform_method!(transform_node, Node, Node);
    transform_method!(
        transform_object_pattern,
        ObjectDestructure<FromOf<Self>>,
        ObjectDestructure<ToOf<Self>>
    );
    transform_method!(
        transform_array_pattern,
        ArrayDestructure<FromOf<Self>>,
        ArrayDestructure<ToOf<Self>>
    );
    transform_method!(transform_enum_pattern, EnumDestructure, EnumDestructure);
    transform_method!(transform_import, Import<FromOf<Self>>, Import<ToOf<Self>>);
    transform_method!(transform_type_node, NodeType, NodeType);
    transform_method!(transform_type_number, NumberType, NumberType);
    transform_method!(transform_type_string, StringType, StringType);
    transform_method!(transform_type_boolean, BooleanType, BooleanType);
    transform_method!(transform_type_none, NoneType, NoneType);
    transform_method!(transform_type_never, NeverType, NeverType);
    transform_method!(
        transform_type_object,
        ObjectType<FromOf<Self>>,
        ObjectType<ToOf<Self>>
    );
    transform_method!(
        transform_type_array,
        ArrayType<FromOf<Self>>,
        ArrayType<ToOf<Self>>
    );
    transform_method!(
        transform_type_optional,
        OptionalType<FromOf<Self>>,
        OptionalType<ToOf<Self>>
    );
    transform_method!(
        transform_type_lambda,
        LambdaType<FromOf<Self>>,
        LambdaType<ToOf<Self>>
    );
    transform_method!(
        transform_type_union,
        UnionType<FromOf<Self>>,
        UnionType<ToOf<Self>>
    );
    transform_method!(
        transform_type_tuple,
        TupleType<FromOf<Self>>,
        TupleType<ToOf<Self>>
    );
    transform_method!(transform_string, String, String);
    transform_method!(transform_boolean, bool, bool);
    transform_method!(transform_number, f64, f64);
    transform_method!(transform_block, Block<FromOf<Self>>, Block<ToOf<Self>>);
    transform_method!(
        transform_expression,
        Expression<FromOf<Self>>,
        Expression<ToOf<Self>>
    );
    transform_method!(
        transform_pattern,
        Pattern<FromOf<Self>>,
        Pattern<ToOf<Self>>
    );
    transform_method!(
        transform_statement,
        Statement<FromOf<Self>>,
        Statement<ToOf<Self>>
    );
    transform_method!(
        transform_program,
        Program<FromOf<Self>>,
        Program<ToOf<Self>>
    );
    transform_method!(transform_type, Type<FromOf<Self>>, Type<ToOf<Self>>);
}

impl<T: AstTransform> AstTraverse for T {}

pub trait AstTransform {
    type Error;
    type From: Ast;
    type To: Ast;
}

impl<A: AstTraverse>
    Transform<Expression<<A as AstTransform>::From>, Expression<<A as AstTransform>::To>> for A
where
    A: Transform<Match<<A as AstTransform>::From>, Match<<A as AstTransform>::To>>
        + Transform<Ident<<A as AstTransform>::From>, Ident<<A as AstTransform>::To>>
        + Transform<Primative, Primative>
        + Transform<Array<<A as AstTransform>::From>, Array<<A as AstTransform>::To>>
        + Transform<Object<<A as AstTransform>::From>, Object<<A as AstTransform>::To>>
        + Transform<Lambda<<A as AstTransform>::From>, Lambda<<A as AstTransform>::To>>
        + Transform<Node, Node>
        + Transform<
            UnaryOperation<<A as AstTransform>::From>,
            UnaryOperation<<A as AstTransform>::To>,
        > + Transform<
            BinaryOperation<<A as AstTransform>::From>,
            BinaryOperation<<A as AstTransform>::To>,
        > + Transform<Call<<A as AstTransform>::From>, Call<<A as AstTransform>::To>>
        + Transform<ObjectAccess<<A as AstTransform>::From>, ObjectAccess<<A as AstTransform>::To>>
        + Transform<Block<<A as AstTransform>::From>, Block<<A as AstTransform>::To>>,
{
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<
            Expression<<A as AstTransform>::From>,
            <A as AstTransform>::From,
        >,
    ) -> Results<AstNode<Expression<<A as AstTransform>::To>, <A as AstTransform>::To>, A::Error>
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
}

impl<A: AstTraverse> Transform<Primative, Primative> for A
where
    A: Transform<bool, bool> + Transform<f64, f64> + Transform<String, String>,
{
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<Primative, <A as AstTransform>::From>,
    ) -> Results<AstNode<Primative, Self::To>, A::Error> {
        match inner {
            Primative::Boolean(x) => self.dispatch(meta, x, Primative::Boolean),
            Primative::Number(x) => self.dispatch(meta, x, Primative::Number),
            Primative::String(x) => self.dispatch(meta, x, Primative::String),
        }
    }
}

impl<A: AstTraverse> Transform<Pattern<<A as AstTransform>::From>, Pattern<<A as AstTransform>::To>>
    for A
where
    A: Transform<
            ObjectDestructure<<A as AstTransform>::From>,
            ObjectDestructure<<A as AstTransform>::To>,
        > + Transform<
            ArrayDestructure<<A as AstTransform>::From>,
            ArrayDestructure<<A as AstTransform>::To>,
        > + Transform<EnumDestructure, EnumDestructure>
        + Transform<Ident<<A as AstTransform>::From>, Ident<<A as AstTransform>::To>>
        + Transform<Primative, Primative>,
{
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<Pattern<Self::From>, Self::From>,
    ) -> Results<AstNode<Pattern<Self::To>, Self::To>, A::Error> {
        match inner {
            Pattern::Object(x) => self.dispatch(meta, x, Pattern::Object),
            Pattern::Array(x) => self.dispatch(meta, x, Pattern::Array),
            Pattern::Enum(x) => self.dispatch(meta, x, Pattern::Enum),
            Pattern::Binding(x) => self.dispatch(meta, x, Pattern::Binding),
            Pattern::Primative(x) => self.dispatch(meta, x, Pattern::Primative),
        }
    }
}
impl<A: AstTraverse>
    Transform<Statement<<A as AstTransform>::From>, Statement<<A as AstTransform>::To>> for A
where
    A: Transform<Definition<<A as AstTransform>::From>, Definition<<A as AstTransform>::To>>
        + Transform<
            TypeDefinition<<A as AstTransform>::From>,
            TypeDefinition<<A as AstTransform>::To>,
        > + Transform<Return<<A as AstTransform>::From>, Return<<A as AstTransform>::To>>
        + Transform<Expression<<A as AstTransform>::From>, Expression<<A as AstTransform>::To>>,
{
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<Statement<Self::From>, Self::From>,
    ) -> Results<AstNode<Statement<Self::To>, Self::To>, A::Error> {
        match inner {
            Statement::Definition(x) => self.dispatch(meta, x, Statement::Definition),
            Statement::TypeDefinition(x) => self.dispatch(meta, x, Statement::TypeDefinition),
            Statement::Return(x) => self.dispatch(meta, x, Statement::Return),
            Statement::Expression(x) => self.dispatch(meta, x, Statement::Expression),
        }
    }
}

impl<A: AstTraverse> Transform<Program<<A as AstTransform>::From>, Program<<A as AstTransform>::To>>
    for A
where
    A: Transform<Import<<A as AstTransform>::From>, Import<<A as AstTransform>::To>>
        + Transform<Definition<<A as AstTransform>::From>, Definition<<A as AstTransform>::To>>
        + Transform<Statement<<A as AstTransform>::From>, Statement<<A as AstTransform>::To>>,
{
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<Program<Self::From>, Self::From>,
    ) -> Results<AstNode<Program<Self::To>, Self::To>, A::Error> {
        match inner {
            Program::Import(x) => self.dispatch(meta, x, Program::Import),
            Program::Export(x) => self.dispatch(meta, x, Program::Export),
            Program::Statement(x) => self.dispatch(meta, x, Program::Statement),
        }
    }
}

impl<A: AstTraverse> Transform<Type<<A as AstTransform>::From>, Type<<A as AstTransform>::To>> for A
where
    A: Transform<NodeType, NodeType>
        + Transform<StringType, StringType>
        + Transform<NumberType, NumberType>
        + Transform<BooleanType, BooleanType>
        + Transform<ObjectType<<A as AstTransform>::From>, ObjectType<<A as AstTransform>::To>>
        + Transform<LambdaType<<A as AstTransform>::From>, LambdaType<<A as AstTransform>::To>>
        + Transform<ArrayType<<A as AstTransform>::From>, ArrayType<<A as AstTransform>::To>>
        + Transform<OptionalType<<A as AstTransform>::From>, OptionalType<<A as AstTransform>::To>>
        + Transform<UnionType<<A as AstTransform>::From>, UnionType<<A as AstTransform>::To>>
        + Transform<NoneType, NoneType>
        + Transform<TupleType<<A as AstTransform>::From>, TupleType<<A as AstTransform>::To>>
        + Transform<NeverType, NeverType>,
{
    fn transform(
        &mut self,
        AstNode { inner, meta }: AstNode<Type<Self::From>, Self::From>,
    ) -> Results<AstNode<Type<Self::To>, Self::To>, A::Error> {
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
