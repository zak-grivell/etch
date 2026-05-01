use ast::{
    Argument, ComponentDefinition, Connection, Expression, Main, Parameter, Port, Statement, Type,
    Varible,
};

#[rust_sitter::grammar("etch")]
mod grammar {
    #[rust_sitter::language]
    #[derive(Debug, Clone, PartialEq)]
    pub struct Program {
        pub items: Vec<Item>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Item {
        Component(ComponentDefinition),
        Constant(ConstantDefinition),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ComponentDefinition {
        #[rust_sitter::leaf(text = "component")]
        _component: (),
        pub name: Identifier,
        pub parameters: ParameterList,
        pub body: ComponentBody,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ParameterList {
        #[rust_sitter::leaf(text = "(")]
        _open: (),
        #[rust_sitter::delimited(
            #[rust_sitter::leaf(text = ",")]
            ()
        )]
        pub parameters: Vec<Parameter>,
        #[rust_sitter::leaf(text = ")")]
        _close: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Parameter {
        pub name: Identifier,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub ty: TypeName,
        pub default: Option<DefaultValue>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct DefaultValue {
        #[rust_sitter::leaf(text = "=")]
        _equals: (),
        pub value: Expression,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ComponentBody {
        #[rust_sitter::leaf(text = "{")]
        _open: (),
        pub statements: Vec<Statement>,
        #[rust_sitter::leaf(text = "}")]
        _close: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Statement {
        Port(PortDefinition),
        Let(LetDefinition),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct PortDefinition {
        #[rust_sitter::leaf(text = "port")]
        _port: (),
        pub name: Identifier,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub ty: TypeName,
        #[rust_sitter::leaf(text = ";")]
        _semi: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct LetDefinition {
        #[rust_sitter::leaf(text = "let")]
        _let: (),
        pub name: Identifier,
        #[rust_sitter::leaf(text = "=")]
        _equals: (),
        pub value: Expression,
        #[rust_sitter::leaf(text = ";")]
        _semi: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ConstantDefinition {
        #[rust_sitter::leaf(text = "constant")]
        _constant: (),
        pub name: Identifier,
        #[rust_sitter::leaf(text = "=")]
        _equals: (),
        pub value: Expression,
        #[rust_sitter::leaf(text = ";")]
        _semi: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Expression {
        ComponentInstance(ComponentInstance),
        Access(Access),
        Quantity(Quantity),
        String(StringLiteral),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ComponentInstance {
        pub name: Identifier,
        pub connections: Option<ConnectionList>,
        pub arguments: ArgumentList,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ConnectionList {
        #[rust_sitter::leaf(text = "<")]
        _open: (),
        pub connections: Vec<TrailingConnection>,
        #[rust_sitter::leaf(text = ">")]
        _close: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TrailingConnection {
        pub connection: Connection,
        pub comma: Option<Comma>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Connection {
        pub name: Identifier,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub target: Access,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ArgumentList {
        #[rust_sitter::leaf(text = "(")]
        _open: (),
        pub arguments: Vec<TrailingArgument>,
        #[rust_sitter::leaf(text = ")")]
        _close: (),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TrailingArgument {
        pub argument: Argument,
        pub comma: Option<Comma>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Argument {
        pub name: Identifier,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub value: Expression,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Access {
        pub head: Identifier,
        pub tail: Vec<AccessSegment>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct AccessSegment {
        #[rust_sitter::leaf(text = ".")]
        _dot: (),
        pub field: Identifier,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Quantity {
        #[rust_sitter::leaf(pattern = r"[0-9]+(\.[0-9]+)?", transform = parse_f64)]
        pub value: f64,
        pub suffix: Option<UnitSuffix>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct StringLiteral {
        #[rust_sitter::leaf(pattern = r#""([^"\\]|\\.)*""#, transform = unquote)]
        pub value: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct TypeName {
        pub name: Identifier,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Identifier {
        #[rust_sitter::word]
        #[rust_sitter::leaf(pattern = r"[A-Za-z_][A-Za-z0-9_]*", transform = str::to_owned)]
        pub value: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct UnitSuffix {
        #[rust_sitter::leaf(pattern = r"[A-Za-z_µΩ]+", transform = str::to_owned)]
        pub value: String,
    }

    #[rust_sitter::leaf(text = ",")]
    #[derive(Debug, Clone, PartialEq)]
    pub struct Comma;

    #[rust_sitter::extra]
    #[allow(dead_code)]
    pub struct Whitespace {
        #[rust_sitter::leaf(pattern = r"\s")]
        _whitespace: (),
    }

    #[rust_sitter::extra]
    #[allow(dead_code)]
    pub struct LineComment {
        #[rust_sitter::leaf(pattern = r"(#|//)[^\n]*")]
        _comment: (),
    }

    fn parse_f64(value: &str) -> f64 {
        value.parse().unwrap()
    }

    fn unquote(value: &str) -> String {
        value[1..value.len() - 1].to_owned()
    }
}

pub fn parse(file: &str) -> Result<Vec<Main>, Vec<rust_sitter::errors::ParseError>> {
    grammar::parse(file).map(|program| program.items.into_iter().map(Into::into).collect())
}

impl From<grammar::Item> for Main {
    fn from(item: grammar::Item) -> Self {
        match item {
            grammar::Item::Component(component) => Main::ComponentDefinition(component.into()),
            grammar::Item::Constant(constant) => Main::Varible(Varible {
                name: constant.name.value,
                value: constant.value.into(),
            }),
        }
    }
}

impl From<grammar::ComponentDefinition> for ComponentDefinition {
    fn from(component: grammar::ComponentDefinition) -> Self {
        Self {
            name: component.name.value,
            parameters: component
                .parameters
                .parameters
                .into_iter()
                .map(Into::into)
                .collect(),
            statements: component
                .body
                .statements
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<grammar::Parameter> for Parameter {
    fn from(parameter: grammar::Parameter) -> Self {
        Self {
            name: parameter.name.value,
            t: parameter.ty.into(),
            default: parameter.default.map(|default| default.value.into()),
        }
    }
}

impl From<grammar::Statement> for Statement {
    fn from(statement: grammar::Statement) -> Self {
        match statement {
            grammar::Statement::Port(port) => Statement::PortDefinition(Port {
                name: port.name.value,
                t: port.ty.into(),
            }),
            grammar::Statement::Let(value) => Statement::VaribleDefinition(Varible {
                name: value.name.value,
                value: value.value.into(),
            }),
        }
    }
}

impl From<grammar::TypeName> for Type {
    fn from(ty: grammar::TypeName) -> Self {
        match ty.name.value.as_str() {
            "Number" => Type::Number {
                min: None,
                max: None,
            },
            "String" => Type::String,
            "Component" => Type::Component,
            "Net" => Type::Net,
            _ => Type::Named(ty.name.value),
        }
    }
}

impl From<grammar::Expression> for Expression {
    fn from(expression: grammar::Expression) -> Self {
        match expression {
            grammar::Expression::ComponentInstance(instance) => instance.into(),
            grammar::Expression::Access(access) => access.into(),
            grammar::Expression::Quantity(quantity) => Expression::Quantity {
                value: quantity.value,
                suffix: quantity.suffix.map(|suffix| suffix.value),
            },
            grammar::Expression::String(string) => Expression::String(string.value),
        }
    }
}

impl From<grammar::ComponentInstance> for Expression {
    fn from(instance: grammar::ComponentInstance) -> Self {
        Expression::ComponentInstance {
            name: instance.name.value,
            connections: instance
                .connections
                .map(|connections| {
                    connections
                        .connections
                        .into_iter()
                        .map(|connection| connection.connection.into())
                        .collect()
                })
                .unwrap_or_default(),
            arguments: instance
                .arguments
                .arguments
                .into_iter()
                .map(|argument| argument.argument.into())
                .collect(),
        }
    }
}

impl From<grammar::Connection> for Connection {
    fn from(connection: grammar::Connection) -> Self {
        Self {
            name: connection.name.value,
            target: connection.target.into(),
        }
    }
}

impl From<grammar::Argument> for Argument {
    fn from(argument: grammar::Argument) -> Self {
        Self {
            name: argument.name.value,
            value: argument.value.into(),
        }
    }
}

impl From<grammar::Access> for Expression {
    fn from(access: grammar::Access) -> Self {
        access.tail.into_iter().fold(
            Expression::Identifier(access.head.value),
            |base, segment| Expression::Access {
                base: Box::new(base),
                field: segment.field.value,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_voltage_divider() {
        let ast = parse(
            "
# this is a comment
component VoltageDivider(r1: Resistance, r2: Resistance) {
    port vcc: Analog;
    port mid: Analog;
    port gnd: Analog;

    let resistor_one = Resistor<a:vcc, b:mid>(value: r1);
    let resistor_two = Resistor<a:mid, b:gnd>(value: r2);
}
",
        )
        .unwrap();

        assert_eq!(ast.len(), 1);
    }

    #[test]
    fn parses_example_file() {
        let ast = parse(include_str!("../../examples/main.etch")).unwrap();

        assert_eq!(ast.len(), 2);
    }
}
