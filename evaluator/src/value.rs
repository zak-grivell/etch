use super::*;

#[derive(Clone)]
pub struct LambdaValue {
    pub(super) params: BTreeMap<Option<Symbol>, ast::Type<PartialMetadata>>,
    pub(super) body: Box<AstNode<Expression<PartialMetadata>, PartialMetadata>>,
    pub(super) scope: Scope,
    pub(super) recursive: Option<Symbol>,
}

impl fmt::Debug for LambdaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LambdaValue")
            .field("params", &self.params.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    Quantity(f64, String),
    String(String),
    Boolean(bool),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
    Lambda(LambdaValue),
    Node(NodeValue),
    NativeFunction(NativeFunction),
    None,
}

#[derive(Clone, Debug)]
pub struct NativeFunction {
    pub(super) kind: NativeFunctionKind,
    pub(super) circuit: super::circuit::CircuitRef,
}

#[derive(Clone, Debug)]
pub(super) enum NativeFunctionKind {
    Hook,
    UseNode,
    UseState,
    UseEquation,
    Voltage,
    Conductance,
    Current,
    FixVoltage,
    DriveVoltage,
    Time,
    DeltaTime,
    StateGet(u64),
    StateSet(u64),
    Test,
    Assert,
    AssertClose,
    Simulate,
    UseSymbol,
    Section,
    Net,
    Display,
    PcbConfig,
    Render,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::Quantity(a, a_unit), Self::Quantity(b, b_unit)) => a == b && a_unit == b_unit,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Boolean(a), Self::Boolean(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Node(a), Self::Node(b)) => a == b,
            (Self::None, Self::None) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationError {
    pub span: Span,
    pub message: String,
}

pub trait SourceProvider {
    fn main_file(&self) -> &str;
    fn get_file(&self, name: &str) -> Option<&str>;
    fn read_file(&self, name: &str) -> Result<String, RunError> {
        self.get_file(name)
            .map(str::to_owned)
            .ok_or_else(|| RunError::FileNotFound(name.into()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RunError {
    FileNotFound(String),
    Io {
        file: String,
        message: String,
    },
    ImportCycle(Vec<String>),
    Compilation {
        file: String,
        diagnostics: Vec<parser::CompileDiagnostic>,
    },
    Evaluation(EvaluationError),
}

#[derive(Clone, Debug)]
pub struct EvaluationOutput {
    pub values: Vec<Value>,
    pub circuit: Circuit,
}

impl EvaluationOutput {
    pub fn rendered_component(&self) -> Option<Value> {
        self.circuit.rendered_component()
    }

    pub fn run_tests(&self) -> Vec<TestResult> {
        self.circuit.run_tests()
    }

    pub fn design(&self) -> CircuitDesign {
        self.circuit.design()
    }

    pub fn run_displays(&self) -> Vec<DisplayResult> {
        self.circuit.run_displays()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestResult {
    pub name: String,
    pub result: Result<(), EvaluationError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayResult {
    pub name: String,
    pub result: Result<DisplayOutput, EvaluationError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayOutput {
    pub traces: Vec<TraceSeries>,
}

pub use circuit_ir::TraceSeries;

impl From<EvaluationError> for RunError {
    fn from(error: EvaluationError) -> Self {
        Self::Evaluation(error)
    }
}

type Values = Rc<RefCell<BTreeMap<Symbol, Value>>>;

#[derive(Clone, Default)]
pub(super) struct Scope {
    pub(super) values: Values,
}

impl Scope {
    pub(super) fn child(&self) -> Self {
        Self {
            values: Rc::new(RefCell::new(self.values.borrow().clone())),
        }
    }

    pub(super) fn get(&self, symbol: &Symbol) -> Option<Value> {
        self.values.borrow().get(symbol).cloned()
    }

    pub(super) fn set(&self, symbol: Symbol, value: Value) {
        self.values.borrow_mut().insert(symbol, value);
    }
}

pub(super) enum Flow {
    Continue(Value),
    Return(Value),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(file) => write!(f, "source file not found: {file}"),
            Self::Io { file, message } => write!(f, "{file}: {message}"),
            Self::ImportCycle(files) => write!(f, "import cycle: {}", files.join(" -> ")),
            Self::Compilation { file, diagnostics } => {
                for diagnostic in diagnostics {
                    writeln!(
                        f,
                        "{file}:{}..{}: {}: {}",
                        diagnostic.span.start,
                        diagnostic.span.end,
                        diagnostic.stage,
                        diagnostic.message
                    )?;
                }
                Ok(())
            }
            Self::Evaluation(error) => write!(
                f,
                "{}..{}: {}",
                error.span.start, error.span.end, error.message
            ),
        }
    }
}
impl std::error::Error for RunError {}

impl Value {
    pub(super) fn retain_circuit(self, circuit: &Circuit) -> Self {
        match self {
            Self::Node(mut node) => {
                node.owner = Some(circuit.clone());
                Self::Node(node)
            }
            Self::Array(items) => Self::Array(
                items
                    .into_iter()
                    .map(|item| item.retain_circuit(circuit))
                    .collect(),
            ),
            Self::Object(fields) => Self::Object(
                fields
                    .into_iter()
                    .map(|(key, value)| (key, value.retain_circuit(circuit)))
                    .collect(),
            ),
            other => other,
        }
    }
}
