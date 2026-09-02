use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use ast::{
    AstNode, BinaryOperator, Expression, Pattern, Primative, Program, Span, Statement,
    UnaryOperator,
};
use parser::{PartialMetadata, Symbol, TypedProgram};

mod graph;
mod schematic;

#[cfg(test)]
mod test;

#[derive(Clone, Default)]
pub struct Circuit {
    inner: Rc<RefCell<CircuitState>>,
}

#[derive(Clone, Default)]
struct CircuitState {
    next_node_id: u64,
    next_state_id: u64,
    voltages: BTreeMap<u64, f64>,
    connections: BTreeMap<u64, BTreeSet<u64>>,
    hooks: Vec<LambdaValue>,
    conductances: Vec<(u64, u64, f64)>,
    currents: Vec<(u64, u64, f64)>,
    voltage_drives: Vec<(u64, f64, f64)>,
    fixed: BTreeMap<u64, f64>,
    time: f64,
    delta_time: f64,
    states: BTreeMap<u64, StateSlot>,
    tests: Vec<LanguageTest>,
    schematic_components: Vec<SchematicComponent>,
    net_labels: BTreeMap<String, Vec<u64>>,
    sections: Vec<String>,
    current_section: Option<String>,
    displays: Vec<LanguageDisplay>,
}

#[derive(Clone, Debug)]
struct StateSlot {
    current: Value,
    pending: Option<Value>,
}

#[derive(Clone, Debug)]
struct LanguageTest {
    name: String,
    body: LambdaValue,
}

#[derive(Clone, Debug)]
struct SchematicComponent {
    kind: String,
    label: Option<String>,
    value: Option<String>,
    ports: BTreeMap<String, u64>,
    section: Option<String>,
    svg: Option<String>,
}

#[derive(Clone, Debug)]
struct LanguageDisplay {
    name: String,
    steps: usize,
    delta_time: f64,
    traces: BTreeMap<String, LambdaValue>,
}

impl fmt::Debug for Circuit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.inner.borrow();
        f.debug_struct("Circuit")
            .field("nodes", &inner.voltages.len())
            .field("hooks", &inner.hooks.len())
            .field("time", &inner.time)
            .finish()
    }
}

impl Circuit {
    fn node(&self) -> NodeValue {
        let mut inner = self.inner.borrow_mut();
        inner.next_node_id += 1;
        let id = inner.next_node_id;
        inner.voltages.insert(id, 0.0);
        inner.connections.insert(id, BTreeSet::new());
        NodeValue {
            id,
            circuit: self.clone(),
        }
    }

    fn connect(&self, a: u64, b: u64) {
        let mut inner = self.inner.borrow_mut();
        inner.connections.entry(a).or_default().insert(b);
        inner.connections.entry(b).or_default().insert(a);
    }

    pub fn node_count(&self) -> usize {
        self.inner.borrow().voltages.len()
    }

    pub fn hook_count(&self) -> usize {
        self.inner.borrow().hooks.len()
    }

    pub fn voltage(&self, node: &NodeValue) -> f64 {
        self.inner
            .borrow()
            .voltages
            .get(&node.id)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn node_voltages(&self) -> Vec<(u64, f64)> {
        self.inner
            .borrow()
            .voltages
            .iter()
            .map(|(id, voltage)| (*id, *voltage))
            .collect()
    }

    pub fn time(&self) -> f64 {
        self.inner.borrow().time
    }

    pub fn test_count(&self) -> usize {
        self.inner.borrow().tests.len()
    }

    pub fn run_tests(&self) -> Vec<TestResult> {
        let baseline = self.inner.borrow().clone();
        let tests = baseline.tests.clone();
        let results = tests
            .into_iter()
            .map(|test| {
                *self.inner.borrow_mut() = baseline.clone();
                TestResult {
                    name: test.name,
                    result: Evaluator::new(self.clone())
                        .call(test.body, BTreeMap::new(), Span::default())
                        .map(|_| ()),
                }
            })
            .collect();
        *self.inner.borrow_mut() = baseline;
        results
    }

    pub fn schematic_svg(&self) -> String {
        schematic::render(&self.inner.borrow())
    }

    pub fn display_count(&self) -> usize {
        self.inner.borrow().displays.len()
    }

    pub fn run_displays(&self) -> Vec<DisplayResult> {
        let baseline = self.inner.borrow().clone();
        let displays = baseline.displays.clone();
        let results = displays
            .into_iter()
            .map(|display| {
                *self.inner.borrow_mut() = baseline.clone();
                DisplayResult {
                    name: display.name.clone(),
                    result: self.run_display(display),
                }
            })
            .collect();
        *self.inner.borrow_mut() = baseline;
        results
    }

    fn run_display(&self, display: LanguageDisplay) -> Result<DisplayOutput, EvaluationError> {
        let mut traces = display
            .traces
            .keys()
            .map(|name| (name.clone(), Vec::with_capacity(display.steps + 1)))
            .collect::<BTreeMap<_, _>>();
        for step in 0..=display.steps {
            let sample_time = self.time();
            for (name, trace) in &display.traces {
                let value = Evaluator::new(self.clone()).call(
                    trace.clone(),
                    BTreeMap::new(),
                    Span::default(),
                )?;
                let Value::Number(value) = value else {
                    return Err(EvaluationError {
                        span: Span::default(),
                        message: format!("display trace `{name}` must return a number"),
                    });
                };
                traces.get_mut(name).unwrap().push((sample_time, value));
            }
            if step < display.steps {
                self.simulate(1, display.delta_time)?;
            }
        }
        let traces = traces
            .into_iter()
            .map(|(name, samples)| TraceSeries { name, samples })
            .collect::<Vec<_>>();
        let svg = graph::render(&display.name, &traces);
        Ok(DisplayOutput { traces, svg })
    }

    pub fn simulate(&self, steps: usize, delta_time: f64) -> Result<(), EvaluationError> {
        for _ in 0..steps {
            {
                let mut inner = self.inner.borrow_mut();
                inner.delta_time = delta_time;
            }
            for _ in 0..16 {
                let hooks = {
                    let mut inner = self.inner.borrow_mut();
                    inner.conductances.clear();
                    inner.currents.clear();
                    inner.voltage_drives.clear();
                    inner.fixed.clear();
                    inner.hooks.clone()
                };
                for hook in hooks {
                    Evaluator::new(self.clone()).call(hook, BTreeMap::new(), Span::default())?;
                }
                self.solve_iteration();
            }
            self.inner.borrow_mut().time += delta_time;
            let mut inner = self.inner.borrow_mut();
            for state in inner.states.values_mut() {
                if let Some(value) = state.pending.take() {
                    state.current = value;
                }
            }
        }
        Ok(())
    }

    fn solve_iteration(&self) {
        let mut inner = self.inner.borrow_mut();
        let ids = inner.voltages.keys().copied().collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        let mut components = Vec::new();
        for root in ids {
            if !visited.insert(root) {
                continue;
            }
            let mut stack = vec![root];
            let mut component = Vec::new();
            while let Some(node) = stack.pop() {
                component.push(node);
                for next in inner.connections.get(&node).into_iter().flatten() {
                    if visited.insert(*next) {
                        stack.push(*next);
                    }
                }
            }
            components.push(component);
        }

        let old = inner.voltages.clone();
        for component in components {
            if let Some(voltage) = component.iter().find_map(|id| inner.fixed.get(id).copied()) {
                for id in component {
                    inner.voltages.insert(id, voltage);
                }
                continue;
            }
            let members = component.iter().copied().collect::<BTreeSet<_>>();
            let mut numerator = 0.0;
            let mut denominator = 0.0;
            for (a, b, conductance) in &inner.conductances {
                if members.contains(a) && !members.contains(b) {
                    numerator += old.get(b).copied().unwrap_or(0.0) * conductance;
                    denominator += conductance;
                } else if members.contains(b) && !members.contains(a) {
                    numerator += old.get(a).copied().unwrap_or(0.0) * conductance;
                    denominator += conductance;
                }
            }
            for (from, to, current) in &inner.currents {
                if members.contains(from) {
                    numerator -= current;
                }
                if members.contains(to) {
                    numerator += current;
                }
            }
            for (node, target, conductance) in &inner.voltage_drives {
                if members.contains(node) {
                    numerator += target * conductance;
                    denominator += conductance;
                }
            }
            if denominator > 0.0 {
                let voltage = numerator / denominator;
                for id in component {
                    inner.voltages.insert(id, voltage);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeValue {
    id: u64,
    circuit: Circuit,
}

impl NodeValue {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn connections(&self) -> BTreeSet<u64> {
        self.circuit
            .inner
            .borrow()
            .connections
            .get(&self.id)
            .cloned()
            .unwrap_or_default()
    }

    fn connect(&self, other: &Self) {
        self.circuit.connect(self.id, other.id);
    }
}

impl PartialEq for NodeValue {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Clone)]
pub struct LambdaValue {
    params: BTreeMap<Option<Symbol>, ast::Type<PartialMetadata>>,
    body: Box<AstNode<Expression<PartialMetadata>, PartialMetadata>>,
    scope: Scope,
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
    kind: NativeFunctionKind,
    circuit: Circuit,
}

#[derive(Clone, Debug)]
enum NativeFunctionKind {
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
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a == b,
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
    pub fn run_tests(&self) -> Vec<TestResult> {
        self.circuit.run_tests()
    }

    pub fn schematic_svg(&self) -> String {
        self.circuit.schematic_svg()
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
    pub svg: String,
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
struct Scope {
    values: Values,
}

impl Scope {
    fn child(&self) -> Self {
        Self {
            values: Rc::new(RefCell::new(self.values.borrow().clone())),
        }
    }

    fn get(&self, symbol: &Symbol) -> Option<Value> {
        self.values.borrow().get(symbol).cloned()
    }

    fn set(&self, symbol: Symbol, value: Value) {
        self.values.borrow_mut().insert(symbol, value);
    }
}

enum Flow {
    Continue(Value),
    Return(Value),
}

pub fn evaluate(sources: &impl SourceProvider) -> Result<EvaluationOutput, RunError> {
    let mut loading = Vec::new();
    load_file(
        sources,
        sources.main_file(),
        &mut loading,
        Circuit::default(),
    )
    .map(|(output, _)| output)
}

pub fn evaluate_typed(programs: &[TypedProgram]) -> Result<EvaluationOutput, EvaluationError> {
    Evaluator::default().evaluate_typed(programs)
}

fn load_file(
    sources: &impl SourceProvider,
    name: &str,
    loading: &mut Vec<String>,
    circuit: Circuit,
) -> Result<(EvaluationOutput, BTreeMap<String, Value>), RunError> {
    if let Some(index) = loading.iter().position(|file| file == name) {
        let mut cycle = loading[index..].to_vec();
        cycle.push(name.to_owned());
        return Err(RunError::ImportCycle(cycle));
    }
    let source = standard_library_source(name)
        .or_else(|| sources.get_file(name))
        .ok_or_else(|| RunError::FileNotFound(name.to_owned()))?;
    let programs = parser::compile(source).map_err(|errors| RunError::Compilation {
        file: name.to_owned(),
        error_count: errors.len(),
    })?;

    loading.push(name.to_owned());
    let result = Evaluator::new(circuit).evaluate_file(&programs, sources, loading);
    loading.pop();
    result
}

fn standard_library_source(name: &str) -> Option<&'static str> {
    match name {
        "std/sources.txt" => Some(include_str!("../std/sources.txt")),
        "std/passive.txt" => Some(include_str!("../std/passive.txt")),
        "std/analog.txt" => Some(include_str!("../std/analog.txt")),
        "std/digital.txt" => Some(include_str!("../std/digital.txt")),
        _ => None,
    }
}

pub struct Evaluator {
    scope: Scope,
    circuit: Circuit,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new(Circuit::default())
    }
}

impl Evaluator {
    fn new(circuit: Circuit) -> Self {
        let scope = Scope::default();
        scope.set(
            Symbol {
                id: 0,
                name: "hook".into(),
            },
            Value::NativeFunction(NativeFunction {
                kind: NativeFunctionKind::Hook,
                circuit: circuit.clone(),
            }),
        );
        let sim = [
            ("voltage", NativeFunctionKind::Voltage),
            ("conductance", NativeFunctionKind::Conductance),
            ("current", NativeFunctionKind::Current),
            ("fix_voltage", NativeFunctionKind::FixVoltage),
            ("drive_voltage", NativeFunctionKind::DriveVoltage),
            ("time", NativeFunctionKind::Time),
            ("delta_time", NativeFunctionKind::DeltaTime),
        ]
        .into_iter()
        .map(|(name, kind)| {
            (
                name.into(),
                Value::NativeFunction(NativeFunction {
                    kind,
                    circuit: circuit.clone(),
                }),
            )
        })
        .collect();
        scope.set(
            Symbol {
                id: 1,
                name: "sim".into(),
            },
            Value::Object(sim),
        );
        let builtins = [
            (2, "use_node", NativeFunctionKind::UseNode),
            (3, "use_state", NativeFunctionKind::UseState),
            (4, "use_equation", NativeFunctionKind::UseEquation),
            (5, "voltage", NativeFunctionKind::Voltage),
            (6, "conductance", NativeFunctionKind::Conductance),
            (7, "current", NativeFunctionKind::Current),
            (8, "fix_voltage", NativeFunctionKind::FixVoltage),
            (9, "drive_voltage", NativeFunctionKind::DriveVoltage),
            (10, "time", NativeFunctionKind::Time),
            (11, "delta_time", NativeFunctionKind::DeltaTime),
            (12, "test", NativeFunctionKind::Test),
            (13, "assert", NativeFunctionKind::Assert),
            (14, "assert_close", NativeFunctionKind::AssertClose),
            (15, "simulate", NativeFunctionKind::Simulate),
            (16, "use_symbol", NativeFunctionKind::UseSymbol),
            (17, "section", NativeFunctionKind::Section),
            (18, "net", NativeFunctionKind::Net),
            (19, "display", NativeFunctionKind::Display),
        ];
        for (id, name, kind) in builtins {
            scope.set(
                Symbol {
                    id,
                    name: name.into(),
                },
                Value::NativeFunction(NativeFunction {
                    kind,
                    circuit: circuit.clone(),
                }),
            );
        }
        Self { scope, circuit }
    }

    pub fn evaluate_typed(
        &mut self,
        programs: &[TypedProgram],
    ) -> Result<EvaluationOutput, EvaluationError> {
        let mut output = Vec::new();
        for program in programs {
            match &program.inner {
                Program::Import(import) => {
                    return Err(self.error(
                        program.meta.span,
                        format!("runtime imports are not implemented for `{}`", import.path),
                    ));
                }
                Program::Export(definition) => {
                    match self.statement(
                        &Statement::Definition(definition.clone()),
                        program.meta.span,
                    )? {
                        Flow::Continue(Value::None) => {}
                        Flow::Continue(value) => output.push(value),
                        Flow::Return(_) => unreachable!("a definition cannot return"),
                    }
                }
                Program::Statement(statement) => {
                    match self.statement(statement, program.meta.span)? {
                        Flow::Continue(Value::None) => {}
                        Flow::Continue(value) => output.push(value),
                        Flow::Return(_) => {
                            return Err(self.error(program.meta.span, "return outside a lambda"));
                        }
                    }
                }
            }
        }
        Ok(EvaluationOutput {
            values: output,
            circuit: self.circuit.clone(),
        })
    }

    fn evaluate_file(
        &mut self,
        programs: &[TypedProgram],
        sources: &impl SourceProvider,
        loading: &mut Vec<String>,
    ) -> Result<(EvaluationOutput, BTreeMap<String, Value>), RunError> {
        let mut output = Vec::new();
        let mut exports = BTreeMap::new();
        for program in programs {
            match &program.inner {
                Program::Import(import) => {
                    let (_, exports) =
                        load_file(sources, &import.path, loading, self.circuit.clone())?;
                    self.bind(&import.imports, &Value::Object(exports))?;
                }
                Program::Export(definition) => {
                    match self.statement(
                        &Statement::Definition(definition.clone()),
                        program.meta.span,
                    )? {
                        Flow::Continue(Value::None) => {}
                        Flow::Continue(value) => output.push(value),
                        Flow::Return(_) => unreachable!("a definition cannot return"),
                    }
                    self.collect_exports(&definition.lhs, &mut exports);
                }
                Program::Statement(statement) => {
                    match self.statement(statement, program.meta.span)? {
                        Flow::Continue(Value::None) => {}
                        Flow::Continue(value) => output.push(value),
                        Flow::Return(_) => {
                            return Err(self
                                .error(program.meta.span, "return outside a lambda")
                                .into());
                        }
                    }
                }
            }
        }
        Ok((
            EvaluationOutput {
                values: output,
                circuit: self.circuit.clone(),
            },
            exports,
        ))
    }

    fn collect_exports(
        &self,
        pattern: &AstNode<Pattern<PartialMetadata>, PartialMetadata>,
        exports: &mut BTreeMap<String, Value>,
    ) {
        match &pattern.inner {
            Pattern::Binding(ident) => {
                if let Some(symbol) = &ident.ident
                    && let Some(value) = self.scope.get(symbol)
                {
                    exports.insert(symbol.name.clone(), value);
                }
            }
            Pattern::Object(object) => {
                for pattern in object.fields.values() {
                    self.collect_exports(pattern, exports);
                }
            }
            Pattern::Array(array) => {
                for pattern in &array.values {
                    self.collect_exports(pattern, exports);
                }
            }
            Pattern::Enum(_) | Pattern::Primative(_) => {}
        }
    }

    fn error(&self, span: Span, message: impl Into<String>) -> EvaluationError {
        EvaluationError {
            span,
            message: message.into(),
        }
    }

    fn statement(
        &mut self,
        statement: &Statement<PartialMetadata>,
        _span: Span,
    ) -> Result<Flow, EvaluationError> {
        match statement {
            Statement::Definition(definition) => {
                let value = self.expression_node(&definition.rhs)?;
                self.bind(&definition.lhs, &value)?;
                Ok(Flow::Continue(Value::None))
            }
            Statement::TypeDefinition(_) => Ok(Flow::Continue(Value::None)),
            Statement::Return(rtn) => Ok(Flow::Return(self.expression_node(&rtn.value)?)),
            Statement::Expression(expression) => self.expression(expression).map(Flow::Continue),
        }
    }

    fn expression_node(
        &mut self,
        node: &AstNode<Expression<PartialMetadata>, PartialMetadata>,
    ) -> Result<Value, EvaluationError> {
        self.expression(&node.inner)
    }

    fn expression(
        &mut self,
        expression: &Expression<PartialMetadata>,
    ) -> Result<Value, EvaluationError> {
        match expression {
            Expression::Primative(value) => Ok(match value {
                Primative::Boolean(value) => Value::Boolean(*value),
                Primative::Number(value) => Value::Number(*value),
                Primative::String(value) => Value::String(value.clone()),
            }),
            Expression::Ident(ident) => ident
                .ident
                .as_ref()
                .and_then(|symbol| self.scope.get(symbol))
                .ok_or_else(|| self.error(Span::default(), "unresolved value reached evaluator")),
            Expression::Array(array) => array
                .items
                .iter()
                .map(|value| self.expression_node(value))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Expression::Object(object) => object
                .fields
                .iter()
                .map(|(name, value)| {
                    self.expression_node(value)
                        .map(|value| (name.clone(), value))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map(Value::Object),
            Expression::Lambda(lambda) => Ok(Value::Lambda(LambdaValue {
                params: lambda.params.clone(),
                body: lambda.body.clone(),
                scope: self.scope.clone(),
            })),
            Expression::Node(_) => Ok(Value::Node(self.circuit.node())),
            Expression::UnaryOperation(operation) => {
                let value = self.expression_node(&operation.arg)?;
                match (operation.op.clone(), value) {
                    (UnaryOperator::Negate, Value::Number(value)) => Ok(Value::Number(-value)),
                    (UnaryOperator::Flip, Value::Boolean(value)) => Ok(Value::Boolean(!value)),
                    _ => Err(self.error(
                        operation.arg.meta.span,
                        "invalid unary operation at runtime",
                    )),
                }
            }
            Expression::BinaryOperation(operation) => {
                let lhs = self.expression_node(&operation.lhs)?;
                let rhs = self.expression_node(&operation.rhs)?;
                self.binary(operation.op.clone(), lhs, rhs, operation.lhs.meta.span)
            }
            Expression::Call(call) => {
                let callee = self.expression_node(&call.expression)?;
                let mut args = BTreeMap::new();
                for (name, expression) in &call.args {
                    args.insert(name.clone(), self.expression_node(expression)?);
                }
                match callee {
                    Value::Lambda(lambda) => self.call(lambda, args, call.expression.meta.span),
                    Value::NativeFunction(function) => {
                        self.call_native(function, args, call.expression.meta.span)
                    }
                    _ => Err(self.error(
                        call.expression.meta.span,
                        "attempted to call a non-lambda value",
                    )),
                }
            }
            Expression::ObjectAccess(access) => {
                let value = self.expression_node(&access.expression)?;
                let Value::Object(fields) = value else {
                    return Err(self.error(
                        access.expression.meta.span,
                        "field access requires an object",
                    ));
                };
                fields.get(&access.field).cloned().ok_or_else(|| {
                    self.error(
                        access.expression.meta.span,
                        format!("object has no field `{}`", access.field),
                    )
                })
            }
            Expression::Block(block) => self.block(&block.body),
            Expression::Match(mtch) => {
                let value = self.expression_node(&mtch.on)?;
                for arm in &mtch.arms {
                    let mut evaluator = Self {
                        scope: self.scope.child(),
                        circuit: self.circuit.clone(),
                    };
                    let matched = match &arm.pattern {
                        Some(pattern) => evaluator.pattern_matches(pattern, &value)?,
                        None => true,
                    };
                    if !matched {
                        continue;
                    }
                    let guarded = match &arm.condition {
                        Some(condition) => {
                            matches!(evaluator.expression_node(condition)?, Value::Boolean(true))
                        }
                        None => true,
                    };
                    if guarded {
                        return evaluator.expression_node(&arm.result);
                    }
                }
                Ok(Value::None)
            }
        }
    }

    fn block(
        &mut self,
        statements: &[AstNode<Statement<PartialMetadata>, PartialMetadata>],
    ) -> Result<Value, EvaluationError> {
        let parent = self.scope.clone();
        self.scope = self.scope.child();
        let mut last = Value::None;
        for statement in statements {
            match self.statement(&statement.inner, statement.meta.span)? {
                Flow::Continue(value) => last = value,
                Flow::Return(value) => {
                    self.scope = parent;
                    return Ok(value);
                }
            }
        }
        self.scope = parent;
        Ok(last)
    }

    fn call(
        &self,
        lambda: LambdaValue,
        mut args: BTreeMap<String, Value>,
        span: Span,
    ) -> Result<Value, EvaluationError> {
        let mut evaluator = Self {
            scope: lambda.scope.child(),
            circuit: self.circuit.clone(),
        };
        for symbol in lambda.params.keys().flatten() {
            let Some(value) = args.remove(&symbol.name) else {
                return Err(self.error(span, format!("missing argument `{}`", symbol.name)));
            };
            evaluator.scope.set(symbol.clone(), value);
        }
        if let Some(name) = args.keys().next() {
            return Err(self.error(span, format!("unexpected argument `{name}`")));
        }
        evaluator.expression_node(&lambda.body)
    }

    fn call_native(
        &self,
        function: NativeFunction,
        mut args: BTreeMap<String, Value>,
        span: Span,
    ) -> Result<Value, EvaluationError> {
        let missing = |name: &str| self.error(span, format!("missing argument `{name}`"));
        let node = |value: Option<Value>, name: &str| match value {
            Some(Value::Node(node)) => Ok(node),
            Some(_) => Err(self.error(span, format!("argument `{name}` must be a node"))),
            None => Err(missing(name)),
        };
        let number = |value: Option<Value>, name: &str| match value {
            Some(Value::Number(value)) => Ok(value),
            Some(_) => Err(self.error(span, format!("argument `{name}` must be a number"))),
            None => Err(missing(name)),
        };

        match function.kind {
            NativeFunctionKind::Hook => {
                let Some(Value::Lambda(simulate)) = args.remove("simulate") else {
                    return Err(self.error(span, "hook argument `simulate` must be a lambda"));
                };
                let Some(nodes) = args.remove("nodes") else {
                    return Err(missing("nodes"));
                };
                self.ensure_nodes(&nodes, span)?;
                function.circuit.inner.borrow_mut().hooks.push(simulate);
                Ok(Value::None)
            }
            NativeFunctionKind::UseNode => {
                self.ensure_no_args(&args, span)?;
                Ok(Value::Node(function.circuit.node()))
            }
            NativeFunctionKind::UseState => {
                let initial = args.remove("initial").ok_or_else(|| missing("initial"))?;
                self.ensure_no_args(&args, span)?;
                let id = {
                    let mut circuit = function.circuit.inner.borrow_mut();
                    circuit.next_state_id += 1;
                    let id = circuit.next_state_id;
                    circuit.states.insert(
                        id,
                        StateSlot {
                            current: initial,
                            pending: None,
                        },
                    );
                    id
                };
                Ok(Value::Object(BTreeMap::from([
                    (
                        "get".into(),
                        Value::NativeFunction(NativeFunction {
                            kind: NativeFunctionKind::StateGet(id),
                            circuit: function.circuit.clone(),
                        }),
                    ),
                    (
                        "set".into(),
                        Value::NativeFunction(NativeFunction {
                            kind: NativeFunctionKind::StateSet(id),
                            circuit: function.circuit,
                        }),
                    ),
                ])))
            }
            NativeFunctionKind::UseEquation => {
                let Some(Value::Lambda(equation)) = args.remove("equation") else {
                    return Err(
                        self.error(span, "use_equation argument `equation` must be a lambda")
                    );
                };
                self.ensure_no_args(&args, span)?;
                function.circuit.inner.borrow_mut().hooks.push(equation);
                Ok(Value::None)
            }
            NativeFunctionKind::Voltage => {
                let node = node(args.remove("node"), "node")?;
                Ok(Value::Number(function.circuit.voltage(&node)))
            }
            NativeFunctionKind::Conductance => {
                let between = args.remove("between").ok_or_else(|| missing("between"))?;
                let Value::Array(nodes) = between else {
                    return Err(self.error(span, "argument `between` must be a two-node array"));
                };
                if nodes.len() != 2 {
                    return Err(self.error(span, "argument `between` must contain two nodes"));
                }
                let a = node(nodes.first().cloned(), "between[0]")?;
                let b = node(nodes.get(1).cloned(), "between[1]")?;
                let value = number(args.remove("value"), "value")?;
                function
                    .circuit
                    .inner
                    .borrow_mut()
                    .conductances
                    .push((a.id, b.id, value));
                Ok(Value::None)
            }
            NativeFunctionKind::Current => {
                let (from, to) = if let Some(between) = args.remove("between") {
                    let Value::Array(nodes) = between else {
                        return Err(self.error(span, "argument `between` must be a two-node array"));
                    };
                    if nodes.len() != 2 {
                        return Err(self.error(span, "argument `between` must contain two nodes"));
                    }
                    (
                        node(nodes.first().cloned(), "between[0]")?,
                        node(nodes.get(1).cloned(), "between[1]")?,
                    )
                } else {
                    (
                        node(args.remove("from"), "from")?,
                        node(args.remove("to"), "to")?,
                    )
                };
                let value = number(args.remove("value"), "value")?;
                function
                    .circuit
                    .inner
                    .borrow_mut()
                    .currents
                    .push((from.id, to.id, value));
                Ok(Value::None)
            }
            NativeFunctionKind::FixVoltage => {
                let node = node(args.remove("node"), "node")?;
                let value = number(args.remove("value"), "value")?;
                function
                    .circuit
                    .inner
                    .borrow_mut()
                    .fixed
                    .insert(node.id, value);
                Ok(Value::None)
            }
            NativeFunctionKind::DriveVoltage => {
                let node = node(args.remove("node"), "node")?;
                let value = number(args.remove("value"), "value")?;
                let conductance = number(args.remove("conductance"), "conductance")?;
                if !conductance.is_finite() || conductance <= 0.0 {
                    return Err(
                        self.error(span, "argument `conductance` must be positive and finite")
                    );
                }
                function.circuit.inner.borrow_mut().voltage_drives.push((
                    node.id,
                    value,
                    conductance,
                ));
                Ok(Value::None)
            }
            NativeFunctionKind::Time => {
                self.ensure_no_args(&args, span)?;
                Ok(Value::Number(function.circuit.inner.borrow().time))
            }
            NativeFunctionKind::DeltaTime => {
                self.ensure_no_args(&args, span)?;
                Ok(Value::Number(function.circuit.inner.borrow().delta_time))
            }
            NativeFunctionKind::StateGet(id) => {
                self.ensure_no_args(&args, span)?;
                function
                    .circuit
                    .inner
                    .borrow()
                    .states
                    .get(&id)
                    .map(|state| state.current.clone())
                    .ok_or_else(|| self.error(span, "state handle is no longer valid"))
            }
            NativeFunctionKind::StateSet(id) => {
                let value = args.remove("value").ok_or_else(|| missing("value"))?;
                self.ensure_no_args(&args, span)?;
                let mut circuit = function.circuit.inner.borrow_mut();
                let state = circuit
                    .states
                    .get_mut(&id)
                    .ok_or_else(|| self.error(span, "state handle is no longer valid"))?;
                state.pending = Some(value);
                Ok(Value::None)
            }
            NativeFunctionKind::Test => {
                let name = match args.remove("name") {
                    Some(Value::String(name)) => name,
                    Some(_) => {
                        return Err(self.error(span, "test argument `name` must be a string"));
                    }
                    None => return Err(missing("name")),
                };
                let body = match args.remove("body") {
                    Some(Value::Lambda(body)) => body,
                    Some(_) => {
                        return Err(self.error(span, "test argument `body` must be a lambda"));
                    }
                    None => return Err(missing("body")),
                };
                self.ensure_no_args(&args, span)?;
                let mut circuit = function.circuit.inner.borrow_mut();
                if circuit.tests.iter().any(|test| test.name == name) {
                    return Err(self.error(span, format!("duplicate test name `{name}`")));
                }
                circuit.tests.push(LanguageTest { name, body });
                Ok(Value::None)
            }
            NativeFunctionKind::Assert => {
                let condition = match args.remove("condition") {
                    Some(Value::Boolean(condition)) => condition,
                    Some(_) => {
                        return Err(
                            self.error(span, "assert argument `condition` must be a boolean")
                        );
                    }
                    None => return Err(missing("condition")),
                };
                let message = match args.remove("message") {
                    Some(Value::String(message)) => Some(message),
                    Some(_) => {
                        return Err(self.error(span, "assert argument `message` must be a string"));
                    }
                    None => None,
                };
                self.ensure_no_args(&args, span)?;
                if condition {
                    Ok(Value::None)
                } else {
                    Err(self.error(span, message.unwrap_or_else(|| "assertion failed".into())))
                }
            }
            NativeFunctionKind::AssertClose => {
                let actual = number(args.remove("actual"), "actual")?;
                let expected = number(args.remove("expected"), "expected")?;
                let tolerance = number(args.remove("tolerance"), "tolerance")?;
                self.ensure_no_args(&args, span)?;
                if !tolerance.is_finite() || tolerance < 0.0 {
                    return Err(self.error(
                        span,
                        "assert_close argument `tolerance` must be non-negative and finite",
                    ));
                }
                if actual.is_finite()
                    && expected.is_finite()
                    && (actual - expected).abs() <= tolerance
                {
                    Ok(Value::None)
                } else {
                    Err(self.error(
                        span,
                        format!(
                            "assertion failed: expected {expected}, got {actual} (tolerance {tolerance})"
                        ),
                    ))
                }
            }
            NativeFunctionKind::Simulate => {
                let steps = number(args.remove("steps"), "steps")?;
                let delta_time = number(args.remove("delta_time"), "delta_time")?;
                self.ensure_no_args(&args, span)?;
                if !steps.is_finite() || steps < 0.0 || steps.fract() != 0.0 {
                    return Err(self.error(
                        span,
                        "simulate argument `steps` must be a non-negative integer",
                    ));
                }
                if !delta_time.is_finite() || delta_time <= 0.0 {
                    return Err(self.error(
                        span,
                        "simulate argument `delta_time` must be positive and finite",
                    ));
                }
                function.circuit.simulate(steps as usize, delta_time)?;
                Ok(Value::None)
            }
            NativeFunctionKind::UseSymbol => {
                let kind = match args.remove("kind") {
                    Some(Value::String(kind)) => kind,
                    Some(_) => {
                        return Err(self.error(span, "use_symbol argument `kind` must be a string"));
                    }
                    None => return Err(missing("kind")),
                };
                let ports = match args.remove("ports") {
                    Some(Value::Object(ports)) => ports,
                    Some(_) => {
                        return Err(
                            self.error(span, "use_symbol argument `ports` must be an object")
                        );
                    }
                    None => return Err(missing("ports")),
                };
                let mut resolved_ports = BTreeMap::new();
                for (name, value) in ports {
                    let port = node(Some(value), &format!("ports.{name}"))?;
                    if !Rc::ptr_eq(&port.circuit.inner, &function.circuit.inner) {
                        return Err(self.error(span, "symbol port belongs to another circuit"));
                    }
                    resolved_ports.insert(name, port.id);
                }
                let label = self.optional_string(args.remove("label"), "label", span)?;
                let value = args.remove("value").map(|value| self.display_value(&value));
                let svg = self.optional_string(args.remove("svg"), "svg", span)?;
                self.ensure_no_args(&args, span)?;
                let mut circuit = function.circuit.inner.borrow_mut();
                let section = circuit.current_section.clone();
                circuit.schematic_components.push(SchematicComponent {
                    kind,
                    label,
                    value,
                    ports: resolved_ports,
                    section,
                    svg,
                });
                Ok(Value::None)
            }
            NativeFunctionKind::Section => {
                let name = match args.remove("name") {
                    Some(Value::String(name)) => name,
                    Some(_) => {
                        return Err(self.error(span, "section argument `name` must be a string"));
                    }
                    None => return Err(missing("name")),
                };
                let body = match args.remove("body") {
                    Some(Value::Lambda(body)) => body,
                    Some(_) => {
                        return Err(self.error(span, "section argument `body` must be a lambda"));
                    }
                    None => return Err(missing("body")),
                };
                self.ensure_no_args(&args, span)?;
                let previous = {
                    let mut circuit = function.circuit.inner.borrow_mut();
                    if !circuit.sections.contains(&name) {
                        circuit.sections.push(name.clone());
                    }
                    circuit.current_section.replace(name)
                };
                let result = self.call(body, BTreeMap::new(), span);
                function.circuit.inner.borrow_mut().current_section = previous;
                result
            }
            NativeFunctionKind::Net => {
                let label = match args.remove("label") {
                    Some(Value::String(label)) => label,
                    Some(_) => {
                        return Err(self.error(span, "net argument `label` must be a string"));
                    }
                    None => return Err(missing("label")),
                };
                let node = node(args.remove("node"), "node")?;
                self.ensure_no_args(&args, span)?;
                let existing = function
                    .circuit
                    .inner
                    .borrow()
                    .net_labels
                    .get(&label)
                    .and_then(|nodes| nodes.first().copied());
                if let Some(existing) = existing {
                    function.circuit.connect(existing, node.id);
                }
                let mut circuit = function.circuit.inner.borrow_mut();
                let nodes = circuit.net_labels.entry(label).or_default();
                if !nodes.contains(&node.id) {
                    nodes.push(node.id);
                }
                Ok(Value::Node(node))
            }
            NativeFunctionKind::Display => {
                let name = match args.remove("name") {
                    Some(Value::String(name)) => name,
                    Some(_) => {
                        return Err(self.error(span, "display argument `name` must be a string"));
                    }
                    None => return Err(missing("name")),
                };
                let steps = number(args.remove("steps"), "steps")?;
                if !steps.is_finite() || steps < 1.0 || steps.fract() != 0.0 || steps > 1_000_000.0
                {
                    return Err(self.error(
                        span,
                        "display argument `steps` must be an integer from 1 to 1000000",
                    ));
                }
                let delta_time = number(args.remove("delta_time"), "delta_time")?;
                if !delta_time.is_finite() || delta_time <= 0.0 {
                    return Err(self.error(
                        span,
                        "display argument `delta_time` must be positive and finite",
                    ));
                }
                let values = match args.remove("traces") {
                    Some(Value::Object(values)) => values,
                    Some(_) => {
                        return Err(self.error(span, "display argument `traces` must be an object"));
                    }
                    None => return Err(missing("traces")),
                };
                if values.is_empty() {
                    return Err(self.error(span, "display requires at least one trace"));
                }
                let mut traces = BTreeMap::new();
                for (name, value) in values {
                    let Value::Lambda(trace) = value else {
                        return Err(
                            self.error(span, format!("display trace `{name}` must be a lambda"))
                        );
                    };
                    traces.insert(name, trace);
                }
                self.ensure_no_args(&args, span)?;
                let mut circuit = function.circuit.inner.borrow_mut();
                if circuit.displays.iter().any(|display| display.name == name) {
                    return Err(self.error(span, format!("duplicate display name `{name}`")));
                }
                circuit.displays.push(LanguageDisplay {
                    name,
                    steps: steps as usize,
                    delta_time,
                    traces,
                });
                Ok(Value::None)
            }
        }
    }

    fn optional_string(
        &self,
        value: Option<Value>,
        name: &str,
        span: Span,
    ) -> Result<Option<String>, EvaluationError> {
        match value {
            Some(Value::String(value)) => Ok(Some(value)),
            Some(_) => Err(self.error(span, format!("argument `{name}` must be a string"))),
            None => Ok(None),
        }
    }

    fn display_value(&self, value: &Value) -> String {
        match value {
            Value::Number(value) => value.to_string(),
            Value::String(value) => value.clone(),
            Value::Boolean(value) => value.to_string(),
            _ => format!("{value:?}"),
        }
    }

    fn ensure_no_args(
        &self,
        args: &BTreeMap<String, Value>,
        span: Span,
    ) -> Result<(), EvaluationError> {
        if let Some(name) = args.keys().next() {
            Err(self.error(span, format!("unexpected argument `{name}`")))
        } else {
            Ok(())
        }
    }

    fn ensure_nodes(&self, value: &Value, span: Span) -> Result<(), EvaluationError> {
        match value {
            Value::Node(node) if Rc::ptr_eq(&node.circuit.inner, &self.circuit.inner) => Ok(()),
            Value::Node(_) => Err(self.error(span, "hook node belongs to another circuit")),
            Value::Array(values) => values
                .iter()
                .try_for_each(|value| self.ensure_nodes(value, span)),
            Value::Object(values) => values
                .values()
                .try_for_each(|value| self.ensure_nodes(value, span)),
            _ => Err(self.error(span, "hook `nodes` must contain only nodes")),
        }
    }

    fn bind(
        &self,
        pattern: &AstNode<Pattern<PartialMetadata>, PartialMetadata>,
        value: &Value,
    ) -> Result<(), EvaluationError> {
        if self.pattern_matches(pattern, value)? {
            Ok(())
        } else {
            Err(self.error(pattern.meta.span, "value does not match binding pattern"))
        }
    }

    fn pattern_matches(
        &self,
        pattern: &AstNode<Pattern<PartialMetadata>, PartialMetadata>,
        value: &Value,
    ) -> Result<bool, EvaluationError> {
        match (&pattern.inner, value) {
            (Pattern::Binding(ident), value) => {
                if let Some(symbol) = &ident.ident {
                    self.scope.set(symbol.clone(), value.clone());
                }
                Ok(true)
            }
            (Pattern::Primative(Primative::Boolean(a)), Value::Boolean(b)) => Ok(a == b),
            (Pattern::Primative(Primative::Number(a)), Value::Number(b)) => Ok(a == b),
            (Pattern::Primative(Primative::String(a)), Value::String(b)) => Ok(a == b),
            (Pattern::Object(pattern), Value::Object(value)) => {
                for (name, pattern) in &pattern.fields {
                    let Some(value) = value.get(name) else {
                        return Ok(false);
                    };
                    if !self.pattern_matches(pattern, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Pattern::Array(pattern), Value::Array(value))
                if pattern.values.len() == value.len() =>
            {
                for (pattern, value) in pattern.values.iter().zip(value) {
                    if !self.pattern_matches(pattern, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Pattern::Enum(_), _) => Ok(false),
            _ => Ok(false),
        }
    }

    fn binary(
        &self,
        op: BinaryOperator,
        lhs: Value,
        rhs: Value,
        span: Span,
    ) -> Result<Value, EvaluationError> {
        match (op, lhs, rhs) {
            (BinaryOperator::Add, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (BinaryOperator::Sub, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
            (BinaryOperator::Mul, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
            (BinaryOperator::Div, Value::Number(a), Value::Number(b)) if b != 0.0 => {
                Ok(Value::Number(a / b))
            }
            (BinaryOperator::Div, Value::Number(_), Value::Number(_)) => {
                Err(self.error(span, "division by zero"))
            }
            (BinaryOperator::Equal, a, b) => Ok(Value::Boolean(a == b)),
            (BinaryOperator::LessThan, Value::Number(a), Value::Number(b)) => {
                Ok(Value::Boolean(a < b))
            }
            (BinaryOperator::GreaterThan, Value::Number(a), Value::Number(b)) => {
                Ok(Value::Boolean(a > b))
            }
            (BinaryOperator::LessThanOrEqual, Value::Number(a), Value::Number(b)) => {
                Ok(Value::Boolean(a <= b))
            }
            (BinaryOperator::GreaterThanOrEqual, Value::Number(a), Value::Number(b)) => {
                Ok(Value::Boolean(a >= b))
            }
            (BinaryOperator::Union, Value::Object(mut a), Value::Object(b)) => {
                a.extend(b);
                Ok(Value::Object(a))
            }
            (BinaryOperator::Wire, Value::Node(a), Value::Node(b)) => {
                a.connect(&b);
                Ok(Value::Node(a))
            }
            _ => Err(self.error(span, "invalid binary operation at runtime")),
        }
    }
}
