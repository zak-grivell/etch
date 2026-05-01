#[derive(Debug, Clone, PartialEq)]
pub enum Main {
    ComponentDefinition(ComponentDefinition),
    Varible(Varible),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Varible {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDefinition {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub t: Type,
    pub default: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    VaribleDefinition(Varible),
    PortDefinition(Port),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    pub name: String,
    pub t: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // Control Flow
    Switch,
    ForEach,

    // Data
    Number,
    String(String),
    Voltage,
    Current,
    Resistance,
    Range,

    // Operations on data
    Operation,
    Postfix, // mV kO etc

    Identifier(String),
    Access {
        base: Box<Expression>,
        field: String,
    },
    Quantity {
        value: f64,
        suffix: Option<String>,
    },
    ComponentInstance {
        name: String,
        connections: Vec<Connection>,
        arguments: Vec<Argument>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub name: String,
    pub target: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number { min: Option<f64>, max: Option<f64> },
    String,
    Component,
    Net,
    Named(String),
}
