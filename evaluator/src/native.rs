use super::*;

impl Evaluator {
    pub(super) fn call_native(
        &self,
        function: NativeFunction,
        mut args: BTreeMap<String, Value>,
        span: Span,
    ) -> Result<Value, EvaluationError> {
        if matches!(
            function.kind,
            NativeFunctionKind::Hook
                | NativeFunctionKind::UseNode
                | NativeFunctionKind::UseState
                | NativeFunctionKind::UseEquation
        ) && self.function_depth == 0
        {
            return Err(self.error(span, "hooks can only be called inside a function"));
        }
        if matches!(function.kind, NativeFunctionKind::Render) && self.function_depth != 0 {
            return Err(self.error(span, "render can only be called at the top level"));
        }

        let missing = |name: &str| self.error(span, format!("missing argument `{name}`"));
        let node = |value: Option<Value>, name: &str| match value {
            Some(Value::Node(node)) => Ok(node),
            Some(_) => Err(self.error(span, format!("argument `{name}` must be a node"))),
            None => Err(missing(name)),
        };
        let number = |value: Option<Value>, name: &str| match value {
            Some(Value::Number(value)) => Ok(value),
            Some(Value::Quantity(value, _)) => Ok(value),
            Some(_) => Err(self.error(span, format!("argument `{name}` must be a number"))),
            None => Err(missing(name)),
        };

        match function.kind {
            NativeFunctionKind::Render => {
                let component = args
                    .remove("component")
                    .ok_or_else(|| missing("component"))?;
                if !matches!(&component, Value::Object(_)) {
                    return Err(self.error(span, "render argument `component` must be an object"));
                }
                self.ensure_no_args(&args, span)?;
                let mut circuit = function.circuit.inner.borrow_mut();
                if circuit.rendered_component.is_some() {
                    return Err(
                        self.error(span, "a component has already been registered to render")
                    );
                }
                circuit.rendered_component = Some(component);
                Ok(Value::None)
            }
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
                let kicad = match args.remove("kicad") {
                    Some(Value::Object(mut values)) => {
                        let symbol =
                            self.required_string(values.remove("symbol"), "kicad.symbol", span)?;
                        let footprint = self.required_string(
                            values.remove("footprint"),
                            "kicad.footprint",
                            span,
                        )?;
                        let pins = match values.remove("pins") {
                            Some(Value::Object(pins)) => pins,
                            Some(_) => return Err(self.error(span, "kicad.pins must be an object")),
                            None => return Err(self.error(span, "missing argument `kicad.pins`")),
                        };
                        if let Some(name) = values.keys().next() {
                            return Err(
                                self.error(span, format!("unexpected kicad field `{name}`"))
                            );
                        }
                        let mut resolved_pins = BTreeMap::new();
                        for (port, value) in pins {
                            let Value::String(pin) = value else {
                                return Err(self.error(
                                    span,
                                    format!("kicad pin mapping `{port}` must be a string"),
                                ));
                            };
                            if !resolved_ports.contains_key(&port) {
                                return Err(self.error(
                                    span,
                                    format!("kicad pin mapping references unknown port `{port}`"),
                                ));
                            }
                            resolved_pins.insert(port, pin);
                        }
                        if resolved_pins.len() != resolved_ports.len() {
                            return Err(self.error(span, "kicad.pins must map every symbol port"));
                        }
                        Some(KicadLink {
                            symbol,
                            footprint,
                            pins: resolved_pins,
                        })
                    }
                    Some(_) => {
                        return Err(
                            self.error(span, "use_symbol argument `kicad` must be an object")
                        );
                    }
                    None => None,
                };
                self.ensure_no_args(&args, span)?;
                let mut circuit = function.circuit.inner.borrow_mut();
                let section = circuit.current_section.clone();
                circuit.design.add_component(Component {
                    kind,
                    label,
                    value,
                    ports: resolved_ports,
                    section,
                    svg,
                    kicad,
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
                    circuit.design.add_section(name.clone());
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
                    .borrow_mut()
                    .design
                    .add_net_label(label, node.id);
                if let Some(existing) = existing {
                    function.circuit.connect(existing, node.id);
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
            NativeFunctionKind::PcbConfig => {
                let width = number(args.remove("width"), "width")?;
                let height = number(args.remove("height"), "height")?;
                let layers = number(args.remove("layers"), "layers")?;
                let min_trace_width = number(args.remove("min_trace_width"), "min_trace_width")?;
                let clearance = number(args.remove("clearance"), "clearance")?;
                self.ensure_no_args(&args, span)?;
                if !width.is_finite() || width <= 0.0 || !height.is_finite() || height <= 0.0 {
                    return Err(
                        self.error(span, "PCB width and height must be positive and finite")
                    );
                }
                if !layers.is_finite() || layers.fract() != 0.0 || !(1.0..=16.0).contains(&layers) {
                    return Err(self.error(span, "PCB layers must be an integer from 1 to 16"));
                }
                if !min_trace_width.is_finite() || min_trace_width <= 0.0 {
                    return Err(
                        self.error(span, "PCB minimum trace width must be positive and finite")
                    );
                }
                if !clearance.is_finite() || clearance < 0.0 {
                    return Err(self.error(span, "PCB clearance must be non-negative and finite"));
                }
                let config = PcbConfig {
                    width,
                    height,
                    layers: layers as usize,
                    min_trace_width,
                    clearance,
                };
                function
                    .circuit
                    .inner
                    .borrow_mut()
                    .design
                    .configure_pcb(config.clone());
                Ok(Value::Object(BTreeMap::from([
                    ("width".into(), Value::Number(config.width)),
                    ("height".into(), Value::Number(config.height)),
                    ("layers".into(), Value::Number(config.layers as f64)),
                    (
                        "min_trace_width".into(),
                        Value::Number(config.min_trace_width),
                    ),
                    ("clearance".into(), Value::Number(config.clearance)),
                ])))
            }
        }
    }
}
