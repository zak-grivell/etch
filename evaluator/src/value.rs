use super::*;

#[derive(Clone)]
pub struct LambdaValue {
    pub(super) params: BTreeMap<Option<Symbol>, ast::Type<PartialMetadata>>,
    pub(super) body: Box<AstNode<Expression<PartialMetadata>, PartialMetadata>>,
    pub(super) scope: Scope,
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
    pub(super) circuit: Circuit,
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
            (Self::Number(a), Self::Quantity(b, _)) | (Self::Quantity(a, _), Self::Number(b)) => {
                a == b
            }
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum RunError {
    FileNotFound(String),
    ImportCycle(Vec<String>),
    Compilation { file: String, error_count: usize },
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

#[derive(Clone, Debug, PartialEq)]
pub struct TraceSeries {
    pub name: String,
    pub samples: Vec<(f64, f64)>,
}

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
