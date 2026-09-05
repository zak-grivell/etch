//! Small, lossless-value S-expression reader for KiCad library data.
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum SExpr {
    Atom(String),
    String(String),
    List(Vec<Self>),
}
impl SExpr {
    pub fn parse(source: &str) -> Result<Self, String> {
        fn read(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<SExpr, String> {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            match chars.next().ok_or("unexpected end of S-expression")? {
                '(' => {
                    let mut items = Vec::new();
                    loop {
                        while chars.peek().is_some_and(|c| c.is_whitespace()) {
                            chars.next();
                        }
                        if chars.peek() == Some(&')') {
                            chars.next();
                            return Ok(SExpr::List(items));
                        }
                        items.push(read(chars)?);
                    }
                }
                '"' => {
                    let mut value = String::new();
                    loop {
                        match chars.next().ok_or("unterminated string")? {
                            '"' => return Ok(SExpr::String(value)),
                            '\\' => {
                                value.push(match chars.next().ok_or("unterminated escape")? {
                                    'n' => '\n',
                                    'r' => '\r',
                                    't' => '\t',
                                    c => c,
                                })
                            }
                            c => value.push(c),
                        }
                    }
                }
                ')' => Err("unexpected closing parenthesis".into()),
                first => {
                    let mut value = first.to_string();
                    while chars
                        .peek()
                        .is_some_and(|c| !c.is_whitespace() && *c != '(' && *c != ')')
                    {
                        value.push(chars.next().unwrap());
                    }
                    Ok(SExpr::Atom(value))
                }
            }
        }
        let mut chars = source.chars().peekable();
        let result = read(&mut chars)?;
        if chars.any(|c| !c.is_whitespace()) {
            return Err("trailing S-expression data".into());
        }
        Ok(result)
    }
    pub fn atom(value: impl ToString) -> Self {
        Self::Atom(value.to_string())
    }
    pub fn string(value: impl ToString) -> Self {
        Self::String(value.to_string())
    }
    pub fn list(name: &str, values: impl IntoIterator<Item = Self>) -> Self {
        Self::List(std::iter::once(Self::atom(name)).chain(values).collect())
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Atom(v) | Self::String(v) => Some(v),
            _ => None,
        }
    }
    pub fn items(&self) -> &[Self] {
        match self {
            Self::List(v) => v,
            _ => &[],
        }
    }
    pub fn items_mut(&mut self) -> &mut Vec<Self> {
        match self {
            Self::List(v) => v,
            _ => panic!("expected list"),
        }
    }
    pub fn tag(&self) -> Option<&str> {
        self.items().first().and_then(Self::text)
    }
    pub fn field(&self, name: &str) -> Option<&Self> {
        self.items().iter().find(|item| item.tag() == Some(name))
    }
    pub fn value(&self, index: usize) -> Option<&str> {
        self.items().get(index).and_then(Self::text)
    }
    pub fn number(&self, index: usize) -> Result<f64, String> {
        self.value(index)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("invalid numeric field: {self}"))
    }
    pub fn set(&mut self, field: Self) {
        let tag = field.tag().unwrap().to_owned();
        self.items_mut().retain(|item| item.tag() != Some(&tag));
        self.items_mut().push(field);
    }
    pub fn remove(&mut self, name: &str) {
        self.items_mut().retain(|item| item.tag() != Some(name));
    }
    pub fn remove_recursive(&mut self, name: &str) {
        if let Self::List(items) = self {
            items.retain(|item| item.tag() != Some(name));
            for item in items {
                item.remove_recursive(name);
            }
        }
    }
}
impl fmt::Display for SExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Atom(value) => f.write_str(value),
            Self::String(value) => write!(
                f,
                "\"{}\"",
                value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t")
            ),
            Self::List(items) => {
                f.write_str("(")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str(")")
            }
        }
    }
}
