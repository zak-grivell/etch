use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use ast::{
    AstNode, BinaryOperator, Expression, Pattern, Primative, Program, Span, Statement,
    UnaryOperator,
};
use circuit_ir::{CircuitDesign, Component, KicadLink, PcbConfig};
use parser::{PartialMetadata, Symbol, TypedProgram};

mod circuit;
mod native;
mod value;
pub use circuit::{Circuit, NodeValue};
use circuit::{LanguageDisplay, LanguageTest, StateSlot};
pub use value::{
    DisplayOutput, DisplayResult, EvaluationError, EvaluationOutput, LambdaValue, RunError,
    SourceProvider, TestResult, TraceSeries, Value,
};
use value::{Flow, NativeFunction, NativeFunctionKind, Scope};

#[cfg(test)]
mod test;

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
    function_depth: usize,
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
            (20, "pcb_config", NativeFunctionKind::PcbConfig),
            (22, "render", NativeFunctionKind::Render),
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
        scope.set(
            Symbol {
                id: 21,
                name: "ground".into(),
            },
            Value::Node(circuit.ground()),
        );
        Self {
            scope,
            circuit,
            function_depth: 0,
        }
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
                    (UnaryOperator::Negate, Value::Quantity(value, unit)) => {
                        Ok(Value::Quantity(-value, unit))
                    }
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
                        function_depth: self.function_depth,
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
            function_depth: self.function_depth + 1,
        };
        for (symbol, ty) in &lambda.params {
            let Some(symbol) = symbol else { continue };
            let Some(value) = args.remove(&symbol.name) else {
                return Err(self.error(span, format!("missing argument `{}`", symbol.name)));
            };
            let value = match (ty, value) {
                (ast::Type::Number(number), Value::Number(value)) if number.symbol.is_some() => {
                    Value::Quantity(value, number.symbol.clone().unwrap())
                }
                (_, value) => value,
            };
            evaluator.scope.set(symbol.clone(), value);
        }
        if let Some(name) = args.keys().next() {
            return Err(self.error(span, format!("unexpected argument `{name}`")));
        }
        evaluator.expression_node(&lambda.body)
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

    fn required_string(
        &self,
        value: Option<Value>,
        name: &str,
        span: Span,
    ) -> Result<String, EvaluationError> {
        match value {
            Some(Value::String(value)) => Ok(value),
            Some(_) => Err(self.error(span, format!("argument `{name}` must be a string"))),
            None => Err(self.error(span, format!("missing argument `{name}`"))),
        }
    }

    fn display_value(&self, value: &Value) -> String {
        match value {
            Value::Number(value) => value.to_string(),
            Value::Quantity(value, unit) => format!("{value} {unit}"),
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
            (Pattern::Primative(Primative::Number(a)), Value::Quantity(b, _)) => Ok(a == b),
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
        if let (Some((a, a_unit)), Some((b, b_unit))) = (numeric(&lhs), numeric(&rhs)) {
            return match op {
                BinaryOperator::Add | BinaryOperator::Sub => {
                    if a_unit.is_some() && b_unit.is_some() && a_unit != b_unit {
                        Err(self.error(span, "cannot add or subtract different units"))
                    } else {
                        let value = if op == BinaryOperator::Add {
                            a + b
                        } else {
                            a - b
                        };
                        Ok(quantity(value, a_unit.or(b_unit)))
                    }
                }
                BinaryOperator::Mul => Ok(quantity(
                    a * b,
                    match (a_unit, b_unit) {
                        (Some(unit), None) | (None, Some(unit)) => Some(unit),
                        _ => None,
                    },
                )),
                BinaryOperator::Div if b != 0.0 => Ok(quantity(
                    a / b,
                    match (a_unit, b_unit) {
                        (Some(unit), None) => Some(unit),
                        _ => None,
                    },
                )),
                BinaryOperator::Div => Err(self.error(span, "division by zero")),
                BinaryOperator::LessThan => Ok(Value::Boolean(a < b)),
                BinaryOperator::GreaterThan => Ok(Value::Boolean(a > b)),
                BinaryOperator::LessThanOrEqual => Ok(Value::Boolean(a <= b)),
                BinaryOperator::GreaterThanOrEqual => Ok(Value::Boolean(a >= b)),
                BinaryOperator::Equal => Ok(Value::Boolean(a == b && a_unit == b_unit)),
                _ => Err(self.error(span, "invalid binary operation at runtime")),
            };
        }
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

fn numeric(value: &Value) -> Option<(f64, Option<String>)> {
    match value {
        Value::Number(value) => Some((*value, None)),
        Value::Quantity(value, unit) => Some((*value, Some(unit.clone()))),
        _ => None,
    }
}

fn quantity(value: f64, unit: Option<String>) -> Value {
    match unit {
        Some(unit) => Value::Quantity(value, unit),
        None => Value::Number(value),
    }
}
