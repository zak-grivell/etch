use std::collections::BTreeMap;

pub enum Main {
    ComponentDefinition(ComponentDefinition),
    Varible(Varible),
}

pub struct Varible {
    pub name: String,
    pub value: Expression,
}

pub struct ComponentDefinition {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub statements: Vec<Statement>,
}

pub struct Parameter {
    pub name: String,
    pub t: Type,
}

pub enum Statement {
    VaribleDefinition(Varible),
    PortDefinition(Port),
}

pub struct Port {
    pub name: String,
}

pub enum Expression {
    // Control Flow
    Switch,
    ForEach,

    // Data
    Number,
    String,
    Voltage,
    Current,
    Resistance,
    Range,

    // Operations on data
    Operation,
    Postfix, // mV kO etc
}

pub enum Type {
    Number { min: Option<f64>, max: Option<f64> },
    String,
    Component,
    Net,
}
