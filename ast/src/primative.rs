use derive_more::From;

#[derive(Clone, Debug, PartialEq, From)]
pub enum Primative {
    Boolean(bool),
    String(String),
    Number(f64),
}
