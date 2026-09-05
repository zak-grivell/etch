use super::*;

#[derive(Clone, Default)]
pub struct Circuit {
    pub(super) inner: Rc<RefCell<CircuitState>>,
}

#[derive(Clone, Default)]
pub(super) struct CircuitState {
    pub(super) next_node_id: u64,
    pub(super) ground_node_id: Option<u64>,
    pub(super) next_state_id: u64,
    pub(super) voltages: BTreeMap<u64, f64>,
    pub(super) design: CircuitDesign,
    pub(super) hooks: Vec<LambdaValue>,
    pub(super) conductances: Vec<(u64, u64, f64)>,
    pub(super) currents: Vec<(u64, u64, f64)>,
    pub(super) voltage_drives: Vec<(u64, f64, f64)>,
    pub(super) fixed: BTreeMap<u64, f64>,
    pub(super) time: f64,
    pub(super) delta_time: f64,
    pub(super) states: BTreeMap<u64, StateSlot>,
    pub(super) tests: Vec<LanguageTest>,
    pub(super) current_section: Option<String>,
    pub(super) displays: Vec<LanguageDisplay>,
    pub(super) rendered_component: Option<Value>,
}

#[derive(Clone, Debug)]
pub(super) struct StateSlot {
    pub(super) current: Value,
    pub(super) pending: Option<Value>,
}

#[derive(Clone, Debug)]
pub(super) struct LanguageTest {
    pub(super) name: String,
    pub(super) body: LambdaValue,
}

#[derive(Clone, Debug)]
pub(super) struct LanguageDisplay {
    pub(super) name: String,
    pub(super) steps: usize,
    pub(super) delta_time: f64,
    pub(super) traces: BTreeMap<String, LambdaValue>,
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

#[derive(Clone, Debug)]
pub(super) struct CircuitRef(std::rc::Weak<RefCell<CircuitState>>);
impl CircuitRef {
    pub(super) fn upgrade(&self) -> Result<Circuit, EvaluationError> {
        self.0
            .upgrade()
            .map(|inner| Circuit { inner })
            .ok_or_else(|| EvaluationError {
                span: Span::default(),
                message: "the node's circuit has been released".into(),
            })
    }
    pub(super) fn belongs_to(&self, circuit: &Circuit) -> bool {
        self.0.ptr_eq(&Rc::downgrade(&circuit.inner))
    }
}

impl Circuit {
    pub(super) fn downgrade(&self) -> CircuitRef {
        CircuitRef(Rc::downgrade(&self.inner))
    }
    pub(super) fn node(&self) -> NodeValue {
        let mut inner = self.inner.borrow_mut();
        inner.next_node_id += 1;
        let id = inner.next_node_id;
        inner.voltages.insert(id, 0.0);
        inner.design.add_node(id);
        NodeValue {
            id,
            circuit: self.downgrade(),
            owner: None,
        }
    }

    pub(super) fn ground(&self) -> NodeValue {
        if let Some(id) = self.inner.borrow().ground_node_id {
            return NodeValue {
                id,
                circuit: self.downgrade(),
                owner: None,
            };
        }

        let node = self.node();
        let mut inner = self.inner.borrow_mut();
        inner.ground_node_id = Some(node.id);
        inner.design.add_component(Component {
            kind: "ground".into(),
            label: None,
            value: None,
            ports: BTreeMap::from([("node".into(), node.id)]),
            section: None,
            svg: None,
            kicad: Some(KicadLink {
                symbol: "power:GND".into(),
                footprint: String::new(),
                pins: BTreeMap::from([("node".into(), "1".into())]),
            }),
        });
        node
    }

    pub(super) fn connect(&self, a: u64, b: u64) {
        let mut inner = self.inner.borrow_mut();
        inner.design.connect(a, b);
    }

    pub fn node_count(&self) -> usize {
        self.inner.borrow().voltages.len()
    }

    pub fn hook_count(&self) -> usize {
        self.inner.borrow().hooks.len()
    }

    pub fn voltage(&self, node: &NodeValue) -> Result<f64, EvaluationError> {
        if !node.circuit.belongs_to(self) {
            return Err(EvaluationError {
                span: Span::default(),
                message: "node belongs to another circuit".into(),
            });
        }
        self.inner
            .borrow()
            .voltages
            .get(&node.id)
            .copied()
            .ok_or_else(|| EvaluationError {
                span: Span::default(),
                message: "node is no longer present in this circuit".into(),
            })
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

    pub fn rendered_component(&self) -> Option<Value> {
        self.inner
            .borrow()
            .rendered_component
            .clone()
            .map(|value| value.retain_circuit(self))
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

    pub fn design(&self) -> CircuitDesign {
        self.inner.borrow().design.clone()
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
                let Some((value, _)) = numeric(&value) else {
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
        Ok(DisplayOutput { traces })
    }

    pub fn simulate(&self, steps: usize, delta_time: f64) -> Result<(), EvaluationError> {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return Err(EvaluationError {
                span: Span::default(),
                message: "simulation delta time must be positive and finite".into(),
            });
        }
        let baseline = self.inner.borrow().clone();
        let result = self.simulate_steps(steps, delta_time);
        if result.is_err() {
            *self.inner.borrow_mut() = baseline;
        }
        result
    }

    fn simulate_steps(&self, steps: usize, delta_time: f64) -> Result<(), EvaluationError> {
        for _ in 0..steps {
            {
                let mut inner = self.inner.borrow_mut();
                inner.delta_time = delta_time;
            }
            let mut converged = false;
            for _ in 0..128 {
                let previous = self.inner.borrow().voltages.clone();
                let hooks = {
                    let mut inner = self.inner.borrow_mut();
                    inner.conductances.clear();
                    inner.currents.clear();
                    inner.voltage_drives.clear();
                    inner.fixed.clear();
                    if let Some(ground) = inner.ground_node_id {
                        inner.fixed.insert(ground, 0.0);
                    }
                    inner.hooks.clone()
                };
                for hook in hooks {
                    Evaluator::new(self.clone()).call(hook, BTreeMap::new(), Span::default())?;
                }
                self.solve_iteration()?;
                converged = self.inner.borrow().voltages.iter().all(|(node, value)| {
                    let Some(old) = previous.get(node).copied() else {
                        return false;
                    };
                    (value - old).abs() <= 1e-9 + 1e-7 * value.abs().max(old.abs())
                });
                if converged {
                    break;
                }
            }
            if !converged {
                return Err(EvaluationError {
                    span: Span::default(),
                    message: "simulation did not converge within 128 iterations".into(),
                });
            }
            let next_time = self.time() + delta_time;
            if !next_time.is_finite() {
                return Err(EvaluationError {
                    span: Span::default(),
                    message: "simulation time overflow".into(),
                });
            }
            self.inner.borrow_mut().time = next_time;
            let mut inner = self.inner.borrow_mut();
            for state in inner.states.values_mut() {
                if let Some(value) = state.pending.take() {
                    state.current = value;
                }
            }
        }
        Ok(())
    }

    pub(super) fn solve_iteration(&self) -> Result<(), EvaluationError> {
        let mut inner = self.inner.borrow_mut();
        let roots = inner.design.electrical_roots();
        let error = |message: &str| EvaluationError {
            span: Span::default(),
            message: message.into(),
        };
        let root = |id: &u64| {
            roots
                .get(id)
                .copied()
                .ok_or_else(|| error("equation references an unknown node"))
        };
        let mut fixed = BTreeMap::new();
        let mut active = BTreeSet::new();
        for (node, voltage) in &inner.fixed {
            if !voltage.is_finite() {
                return Err(error("non-finite fixed voltage"));
            }
            let node = root(node)?;
            if fixed
                .insert(node, *voltage)
                .is_some_and(|old| old != *voltage)
            {
                return Err(error("connected nodes have conflicting fixed voltages"));
            }
        }
        for (a, b, value) in &inner.conductances {
            if !value.is_finite() || *value < 0.0 {
                return Err(error("conductance must be finite and nonnegative"));
            }
            if *value > 0.0 {
                active.extend([root(a)?, root(b)?]);
            }
        }
        for (a, b, value) in &inner.currents {
            if !value.is_finite() {
                return Err(error("current must be finite"));
            }
            active.extend([root(a)?, root(b)?]);
        }
        for (node, target, g) in &inner.voltage_drives {
            if !target.is_finite() || !g.is_finite() || *g <= 0.0 {
                return Err(error(
                    "voltage drive target must be finite and conductance positive and finite",
                ));
            }
            active.insert(root(node)?);
        }
        let unknowns: BTreeMap<_, _> = active
            .into_iter()
            .filter(|node| !fixed.contains_key(node))
            .enumerate()
            .map(|(index, node)| (node, index))
            .collect();
        let n = unknowns.len();
        let mut matrix = vec![vec![0.0; n + 1]; n];
        for (a, b, g) in &inner.conductances {
            let (a, b) = (root(a)?, root(b)?);
            if a == b || *g == 0.0 {
                continue;
            }
            for (node, other) in [(a, b), (b, a)] {
                if let Some(&i) = unknowns.get(&node) {
                    matrix[i][i] += g;
                    if let Some(&j) = unknowns.get(&other) {
                        matrix[i][j] -= g;
                    } else if let Some(value) = fixed.get(&other) {
                        matrix[i][n] += g * value;
                    }
                }
            }
        }
        for (a, b, current) in &inner.currents {
            if let Some(&i) = unknowns.get(&root(a)?) {
                matrix[i][n] -= current;
            }
            if let Some(&i) = unknowns.get(&root(b)?) {
                matrix[i][n] += current;
            }
        }
        for (node, target, g) in &inner.voltage_drives {
            if let Some(&i) = unknowns.get(&root(node)?) {
                matrix[i][i] += g;
                matrix[i][n] += target * g;
            }
        }
        for row in &mut matrix {
            let scale = row[..n].iter().copied().map(f64::abs).fold(0.0, f64::max);
            if scale == 0.0 {
                return Err(error("floating net: no voltage reference"));
            }
            for value in row {
                *value /= scale;
                if !value.is_finite() {
                    return Err(error("non-finite circuit equation"));
                }
            }
        }
        for column in 0..n {
            let pivot = (column..n)
                .max_by(|a, b| {
                    matrix[*a][column]
                        .abs()
                        .total_cmp(&matrix[*b][column].abs())
                })
                .unwrap();
            if matrix[pivot][column].abs() < 1e-12 {
                return Err(error(
                    "floating or ill-conditioned circuit: no unique voltage solution",
                ));
            }
            matrix.swap(column, pivot);
            let divisor = matrix[column][column];
            for value in &mut matrix[column][column..=n] {
                *value /= divisor;
            }
            let pivot_row = matrix[column].clone();
            for (row_index, row) in matrix.iter_mut().enumerate() {
                if row_index == column {
                    continue;
                }
                let factor = row[column];
                for (value, pivot) in row[column..=n].iter_mut().zip(&pivot_row[column..=n]) {
                    *value -= factor * pivot;
                }
            }
        }
        for (node, index) in unknowns {
            let value = matrix[index][n];
            if !value.is_finite() {
                return Err(error("non-finite voltage solution"));
            }
            fixed.insert(node, value);
        }
        for (node, voltage) in &mut inner.voltages {
            if let Some(value) = fixed.get(&root(node)?) {
                *voltage = *value;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NodeValue {
    pub(super) id: u64,
    pub(super) circuit: CircuitRef,
    pub(super) owner: Option<Circuit>,
}

impl NodeValue {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn connections(&self) -> Result<BTreeSet<u64>, EvaluationError> {
        Ok(self
            .circuit
            .upgrade()?
            .inner
            .borrow()
            .design
            .connections
            .get(&self.id)
            .cloned()
            .unwrap_or_default())
    }
    pub(super) fn connect(&self, other: &Self) -> Result<(), EvaluationError> {
        let circuit = self.circuit.upgrade()?;
        circuit.voltage(self)?;
        circuit.voltage(other)?;
        circuit.connect(self.id, other.id);
        Ok(())
    }
}

impl PartialEq for NodeValue {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.circuit.0.ptr_eq(&other.circuit.0)
    }
}
