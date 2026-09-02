use ast::*;

use crate::semantic::PartialMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanerError {}

pub struct ExpressionStripper;

#[ast::transformer]
impl AstTransform for ExpressionStripper {
    type Error = CleanerError;
    type From = PartialMetadata;
    type To = PartialMetadata;

    identity_generic!(transform_definition, Definition);
    identity_generic!(transform_type_definition, TypeDefinition);
    identity_generic!(transform_return, Return);
    identity_generic!(transform_match, Match);
    identity_generic!(transform_ident, Ident);
    identity_generic!(transform_array, Array);
    identity_generic!(transform_object, Object);
    identity_generic!(transform_lambda, Lambda);
    identity_generic!(transform_unary_op, UnaryOperation);
    identity_generic!(transform_binary_op, BinaryOperation);
    identity_generic!(transform_call, Call);
    identity_generic!(transform_object_access, ObjectAccess);
    identity_leaf!(transform_node, Node);
    identity_generic!(transform_object_pattern, ObjectDestructure);
    identity_generic!(transform_array_pattern, ArrayDestructure);
    identity_leaf!(transform_enum_pattern, EnumDestructure);
    identity_generic!(transform_import, Import);
    identity_leaf!(transform_type_node, NodeType);
    identity_leaf!(transform_type_number, NumberType);
    identity_leaf!(transform_type_string, StringType);
    identity_leaf!(transform_type_boolean, BooleanType);
    identity_leaf!(transform_type_none, NoneType);
    identity_leaf!(transform_type_never, NeverType);
    identity_generic!(transform_type_object, ObjectType);
    identity_generic!(transform_type_array, ArrayType);
    identity_generic!(transform_type_optional, OptionalType);
    identity_generic!(transform_type_lambda, LambdaType);
    identity_generic!(transform_type_union, UnionType);
    identity_generic!(transform_type_tuple, TupleType);
    identity_leaf!(transform_string, String);
    identity_leaf!(transform_boolean, bool);
    identity_leaf!(transform_number, f64);
    identity_generic!(transform_block, Block);
}
