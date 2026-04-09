use std::collections::BTreeMap;

pub enum Main {
    ComponentDefinition {
        name: String,
        parameters: BTreeMap<String, Type>,
        nets: BTreeMap<String, Type>,
        varibles: BTreeMap<String, Expression>,
    },
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
