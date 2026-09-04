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

impl Circuit {
    pub(super) fn node(&self) -> NodeValue {
        let mut inner = self.inner.borrow_mut();
        inner.next_node_id += 1;
        let id = inner.next_node_id;
        inner.voltages.insert(id, 0.0);
        inner.design.add_node(id);
        NodeValue {
            id,
            circuit: self.clone(),
        }
    }

    pub(super) fn ground(&self) -> NodeValue {
        if let Some(id) = self.inner.borrow().ground_node_id {
            return NodeValue {
                id,
                circuit: self.clone(),
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

    pub fn rendered_component(&self) -> Option<Value> {
        self.inner.borrow().rendered_component.clone()
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
                    if let Some(ground) = inner.ground_node_id {
                        inner.fixed.insert(ground, 0.0);
                    }
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
                for next in inner.design.connections.get(&node).into_iter().flatten() {
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
    pub(super) id: u64,
    pub(super) circuit: Circuit,
}

impl NodeValue {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn connections(&self) -> BTreeSet<u64> {
        self.circuit
            .inner
            .borrow()
            .design
            .connections
            .get(&self.id)
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn connect(&self, other: &Self) {
        self.circuit.connect(self.id, other.id);
    }
}

impl PartialEq for NodeValue {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
