use derive_more::From;

#[derive(Clone, Debug, PartialEq, From)]
pub enum Primative {
    Boolean(bool),
    String(String),
    Number(f64),
    Quantity(Quantity),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub unit: String,
}
