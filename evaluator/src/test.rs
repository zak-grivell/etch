use std::collections::BTreeMap;

use crate::{RunError, SourceProvider, Value, evaluate};

struct Sources {
    main: String,
    files: BTreeMap<String, String>,
}

impl Sources {
    fn single(source: &str) -> Self {
        Self {
            main: "main.etch".into(),
            files: BTreeMap::from([("main.etch".into(), source.into())]),
        }
    }
}

impl SourceProvider for Sources {
    fn main_file(&self) -> &str {
        &self.main
    }

    fn get_file(&self, name: &str) -> Option<&str> {
        self.files.get(name).map(String::as_str)
    }
}

fn run(source: &str) -> Vec<Value> {
    evaluate(&Sources::single(source))
        .expect("program should evaluate")
        .values
}

#[test]
fn evaluates_literals_and_arithmetic() {
    assert_eq!(run("1 + 2 * 3"), vec![Value::Number(7.0)]);
    assert_eq!(run("!false"), vec![Value::Boolean(true)]);
}

#[test]
fn evaluates_bindings_objects_and_arrays() {
    assert_eq!(
        run("let x = 2; { x, values: [x, 3] }.values"),
        vec![Value::Array(vec![Value::Number(2.0), Value::Number(3.0)])]
    );
}

#[test]
fn evaluates_pcb_and_kicad_metadata_into_the_design_ir() {
    let source = r#"
        pcb_config(
            width: 30,
            height: 20,
            layers: 2,
            min_trace_width: 0.25,
            clearance: 0.2,
        );
        let Component = () -> {
            let a = use_node();
            let b = use_node();
            use_symbol(kind: "resistor", ports: { a, b }, kicad: {
                symbol: "Device:R",
                footprint: "Resistor_SMD:R_0603_1608Metric",
                pins: { a: "1", b: "2" },
            });
            return { a, b };
        };
        let first = Component();
        let second = Component();
        first.b <- second.a;
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    let design = output.design();
    assert_eq!(design.pcb.as_ref().unwrap().width, 30.0);
    assert_eq!(design.components.len(), 3);
    assert_eq!(
        design
            .components
            .iter()
            .find(|component| component.kind == "resistor")
            .unwrap()
            .kicad
            .as_ref()
            .unwrap()
            .symbol,
        "Device:R"
    );
    assert_eq!(design.electrical_roots().len(), 5);
}

#[test]
fn evaluates_lambdas_and_returns() {
    assert_eq!(
        run("let double = (x: Number) -> { return x * 2 }; double(x: 4)"),
        vec![Value::Number(8.0)]
    );
}

#[test]
fn rejects_top_level_hooks() {
    for source in [
        "use_node()",
        "use_state(initial: 0)",
        "use_equation(equation: () -> 0)",
        "hook(nodes: {}, simulate: () -> 0)",
        "let create_node = use_node; create_node()",
    ] {
        assert!(matches!(
            evaluate(&Sources::single(source)),
            Err(RunError::Evaluation(error))
                if error.message == "hooks can only be called inside a function"
        ));
    }
}

#[test]
fn allows_hooks_scoped_to_function_calls() {
    let output = evaluate(&Sources::single(
        "let Component = () -> use_node(); [Component(), Component()]",
    ))
    .unwrap();

    assert_eq!(output.circuit.node_count(), 3);
}

#[test]
fn provides_one_shared_zero_volt_ground() {
    let output = evaluate(&Sources::single(
        "let GroundReference = () -> ground; [ground, GroundReference()]",
    ))
    .unwrap();
    let Value::Array(nodes) = output.values.last().unwrap() else {
        panic!("expected ground nodes")
    };
    assert_eq!(nodes[0], nodes[1]);
    assert_eq!(
        output
            .design()
            .components
            .iter()
            .filter(|component| component.kind == "ground")
            .count(),
        1
    );

    output.circuit.simulate(1, 0.001).unwrap();
    let Value::Node(ground) = &nodes[0] else {
        panic!("expected a ground node")
    };
    assert_eq!(output.circuit.voltage(ground).unwrap(), 0.0);
}

#[test]
fn registers_an_explicit_component_to_render() {
    let output = evaluate(&Sources::single(
        "let Component = () -> { return { pin: use_node() } }; render(component: Component())",
    ))
    .unwrap();

    assert!(output.values.is_empty());
    assert!(matches!(
        output.rendered_component(),
        Some(Value::Object(component)) if matches!(component.get("pin"), Some(Value::Node(_)))
    ));
}

#[test]
fn does_not_treat_a_file_expression_as_a_render_root() {
    let output = evaluate(&Sources::single("{ pin: ground }")).unwrap();

    assert!(matches!(output.values.last(), Some(Value::Object(_))));
    assert!(output.rendered_component().is_none());
}

#[test]
fn evaluates_matches_and_guards() {
    assert_eq!(
        run("let x = 2; match x { 1 -> 10, let n if n > 1 -> n }"),
        vec![Value::Number(2.0)]
    );
}

#[test]
fn connects_nodes() {
    let values = run("@ <- @");
    let Value::Node(node) = &values[0] else {
        panic!("expected a node")
    };
    assert_eq!(node.connections().unwrap().len(), 1);
}

#[test]
fn evaluates_recursive_lambdas() {
    assert_eq!(
        run(
            "let fact = (n: Number) -> { return match n { 1 -> 1, let n -> n * fact(n: n - 1) } }; fact(n: 5)"
        ),
        vec![Value::Number(120.0)]
    );
}

#[test]
fn loads_imports_from_the_source_provider() {
    let sources = Sources {
        main: "app.etch".into(),
        files: BTreeMap::from([
            (
                "app.etch".into(),
                "from \"math.etch\" import { double }; double(x: 4)".into(),
            ),
            (
                "math.etch".into(),
                "export let double = (x: Number) -> x * 2".into(),
            ),
        ]),
    };
    assert_eq!(evaluate(&sources).unwrap().values, vec![Value::Number(8.0)]);
}

#[test]
fn imports_only_explicitly_exported_values() {
    let sources = Sources {
        main: "app.etch".into(),
        files: BTreeMap::from([
            (
                "app.etch".into(),
                "from \"module.etch\" import { public }; public".into(),
            ),
            (
                "module.etch".into(),
                "let private = 1; export let public = private + 1".into(),
            ),
        ]),
    };
    assert_eq!(evaluate(&sources).unwrap().values, vec![Value::Number(2.0)]);
}

#[test]
fn loads_bundled_standard_library_components() {
    let source = r#"
        from "std/sources.etch" import { VoltageSource, Ground };
        from "std/passive.etch" import { Resistor };
        from "std/analog.etch" import { OpAmp, Led };
        from "std/digital.etch" import { NotGate, DFlipFlop };

        let supply = VoltageSource(value: 5);
        let ground = Ground();
        let upper = Resistor(resistance: 1000);
        let lower = Resistor(resistance: 1000);
        supply.positive <- upper.a;
        upper.b <- lower.a;
        lower.b <- ground.node;

        test(name: "standard divider", body: () -> {
            simulate(steps: 1, delta_time: 0.001);
            assert_close(
                actual: voltage(node: upper.b),
                expected: 2.5,
                tolerance: 0.000001,
            )
        });

        { op_amp: OpAmp, led: Led, inverter: NotGate, flip_flop: DFlipFlop }
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    let tests = output.run_tests();
    assert_eq!(tests.len(), 1);
    assert!(tests[0].result.is_ok());
}

#[test]
fn reports_missing_files() {
    let sources = Sources {
        main: "missing.etch".into(),
        files: BTreeMap::new(),
    };
    assert!(matches!(
        evaluate(&sources),
        Err(RunError::FileNotFound(file)) if file == "missing.etch"
    ));
}

#[test]
fn reports_import_cycles() {
    let sources = Sources {
        main: "a.etch".into(),
        files: BTreeMap::from([
            (
                "a.etch".into(),
                "from \"b.etch\" import { b }; let a = b".into(),
            ),
            (
                "b.etch".into(),
                "from \"a.etch\" import { a }; let b = a".into(),
            ),
        ]),
    };
    assert!(matches!(
        evaluate(&sources),
        Err(RunError::ImportCycle(cycle))
            if cycle == ["a.etch", "b.etch", "a.etch"]
    ));
}

#[test]
fn constructs_and_simulates_component_hooks() {
    let source = r#"
        let FixedVoltage = (voltage: Number) -> {
            let out = @;
            hook(
                nodes: { out },
                simulate: () -> sim.fix_voltage(node: out, value: voltage),
            );
            return { out };
        };

        let Resistor = (resistance: Number) -> {
            let a = @;
            let b = @;
            hook(
                nodes: { a, b },
                simulate: () -> sim.conductance(
                    between: [a, b],
                    value: 1 / resistance,
                ),
            );
            return { a, b };
        };

        let high = FixedVoltage(voltage: 5);
        let low = FixedVoltage(voltage: 0);
        let upper = Resistor(resistance: 1000);
        let lower = Resistor(resistance: 1000);
        high.out <- upper.a;
        upper.b <- lower.a;
        lower.b <- low.out;
        { high: upper.a, middle: upper.b, low: lower.b }
    "#;
    let sources = Sources::single(source);
    if let Err(errors) = parser::compile(source) {
        parser::print_errors("main.etch", source, errors);
        panic!("circuit source did not compile");
    }
    let output = evaluate(&sources).unwrap();
    assert_eq!(output.circuit.node_count(), 7);
    assert_eq!(output.circuit.hook_count(), 4);
    output.circuit.simulate(1, 0.001).unwrap();

    let Value::Object(nodes) = output.values.last().unwrap() else {
        panic!("expected exported circuit nodes")
    };
    let Value::Node(high) = &nodes["high"] else {
        panic!("expected high node")
    };
    let Value::Node(low) = &nodes["low"] else {
        panic!("expected low node")
    };
    let Value::Node(middle) = &nodes["middle"] else {
        panic!("expected middle node")
    };
    assert_eq!(output.circuit.voltage(high).unwrap(), 5.0);
    assert!((output.circuit.voltage(middle).unwrap() - 2.5).abs() < 1e-6);
    assert_eq!(output.circuit.voltage(low).unwrap(), 0.0);
}

#[test]
fn models_analog_equations_and_digital_logic() {
    let source = r#"
        let FixedVoltage = (voltage: Number) -> {
            let out = @;
            hook(
                nodes: { out },
                simulate: () -> sim.fix_voltage(node: out, value: voltage),
            );
            return { out };
        };

        let OpAmp = (gain: Number, low: Number, high: Number) -> {
            let positive = @;
            let negative = @;
            let output = @;
            hook(
                nodes: { positive, negative, output },
                simulate: () -> {
                    let raw = gain * (
                        sim.voltage(node: positive) - sim.voltage(node: negative)
                    );
                    let limited = match raw {
                        let value if value > high -> high,
                        let value if value < low -> low,
                        let value -> value,
                    };
                    sim.drive_voltage(
                        node: output,
                        value: limited,
                        conductance: 1000,
                    )
                },
            );
            return { positive, negative, output };
        };

        let NotGate = (threshold: Number, low: Number, high: Number) -> {
            let input = @;
            let output = @;
            hook(
                nodes: { input, output },
                simulate: () -> {
                    let level = match sim.voltage(node: input) > threshold {
                        true -> low,
                        false -> high,
                    };
                    sim.drive_voltage(
                        node: output,
                        value: level,
                        conductance: 1000,
                    )
                },
            );
            return { input, output };
        };

        let positive = FixedVoltage(voltage: 0.01);
        let negative = FixedVoltage(voltage: 0);
        let high = FixedVoltage(voltage: 5);
        let amplifier = OpAmp(gain: 100, low: -5, high: 5);
        let inverter = NotGate(threshold: 2.5, low: 0, high: 5);
        positive.out <- amplifier.positive;
        negative.out <- amplifier.negative;
        high.out <- inverter.input;
        { analog: amplifier.output, digital: inverter.output }
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    output.circuit.simulate(1, 0.001).unwrap();

    let Value::Object(nodes) = output.values.last().unwrap() else {
        panic!("expected output nodes")
    };
    let Value::Node(analog) = &nodes["analog"] else {
        panic!("expected analog output node")
    };
    let Value::Node(digital) = &nodes["digital"] else {
        panic!("expected digital output node")
    };
    assert!((output.circuit.voltage(analog).unwrap() - 1.0).abs() < 1e-6);
    assert_eq!(output.circuit.voltage(digital).unwrap(), 0.0);
}

#[test]
fn hook_style_state_models_a_capacitor_derivative() {
    let source = r#"
        let Source = (value: Number) -> {
            let out = use_node();
            use_equation(equation: () -> fix_voltage(node: out, value: value));
            return { out };
        };

        let Resistor = (resistance: Number) -> {
            let a = use_node();
            let b = use_node();
            use_equation(equation: () -> conductance(
                between: [a, b],
                value: 1 / resistance,
            ));
            return { a, b };
        };

        let Capacitor = (capacitance: Number) -> {
            let a = use_node();
            let b = use_node();
            let previous_voltage = use_state(initial: 0);
            use_equation(equation: () -> {
                let across = voltage(node: a) - voltage(node: b);
                current(
                    between: [a, b],
                    value: capacitance
                        * (across - previous_voltage.get())
                        / delta_time(),
                );
                previous_voltage.set(value: across)
            });
            return { a, b };
        };

        let supply = Source(value: 1);
        let ground = Source(value: 0);
        let capacitor = Capacitor(capacitance: 0.000001);
        let load = Resistor(resistance: 100);
        supply.out <- capacitor.a;
        capacitor.b <- load.a;
        load.b <- ground.out;
        { output: capacitor.b }
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    let Value::Object(nodes) = output.values.last().unwrap() else {
        panic!("expected output node")
    };
    let Value::Node(node) = &nodes["output"] else {
        panic!("expected output node")
    };

    output.circuit.simulate(1, 0.001).unwrap();
    assert!((output.circuit.voltage(node).unwrap() - 1.0 / 11.0).abs() < 1e-6);
    output.circuit.simulate(1, 0.001).unwrap();
    assert!((output.circuit.voltage(node).unwrap() - 1.0 / 121.0).abs() < 1e-6);
}

#[test]
fn hook_style_state_models_a_rising_edge_flip_flop() {
    let source = r#"
        let Source = (value: Number) -> {
            let out = use_node();
            use_equation(equation: () -> fix_voltage(node: out, value: value));
            return { out };
        };

        let DFlipFlop = () -> {
            let d = use_node();
            let clock = use_node();
            let q = use_node();
            let previous_clock = use_state(initial: false);
            let stored = use_state(initial: false);

            use_equation(equation: () -> {
                let clock_high = voltage(node: clock) > 2.5;
                let rising = match previous_clock.get() {
                    false -> clock_high,
                    true -> false,
                };
                match rising {
                    true -> stored.set(value: voltage(node: d) > 2.5),
                    false -> stored.set(value: stored.get()),
                };
                previous_clock.set(value: clock_high);
                let output = match stored.get() {
                    true -> 5,
                    false -> 0,
                };
                drive_voltage(node: q, value: output, conductance: 1000)
            });
            return { d, clock, q };
        };

        let high_data = Source(value: 5);
        let high_clock = Source(value: 5);
        let flip_flop = DFlipFlop();
        high_data.out <- flip_flop.d;
        high_clock.out <- flip_flop.clock;
        { q: flip_flop.q }
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    let Value::Object(nodes) = output.values.last().unwrap() else {
        panic!("expected output node")
    };
    let Value::Node(q) = &nodes["q"] else {
        panic!("expected q node")
    };

    output.circuit.simulate(1, 0.001).unwrap();
    assert_eq!(output.circuit.voltage(q).unwrap(), 0.0);
    output.circuit.simulate(1, 0.001).unwrap();
    assert_eq!(output.circuit.voltage(q).unwrap(), 5.0);
}

#[test]
fn registers_and_runs_language_tests() {
    let source = r#"
        let Source = (value: Number) -> {
            let out = use_node();
            use_equation(equation: () -> fix_voltage(node: out, value: value));
            return { out };
        };

        let source = Source(value: 3.3);

        test(name: "source reaches its requested voltage", body: () -> {
            simulate(steps: 1, delta_time: 0.001);
            assert_close(
                actual: voltage(node: source.out),
                expected: 3.3,
                tolerance: 0.000001,
            )
        });

        test(name: "failure messages are reported", body: () -> {
            assert(condition: false, message: "deliberate failure")
        });
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    assert_eq!(output.circuit.test_count(), 2);
    assert_eq!(output.circuit.time(), 0.0);

    let results = output.run_tests();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "source reaches its requested voltage");
    assert!(results[0].result.is_ok());
    assert_eq!(results[1].name, "failure messages are reported");
    assert!(matches!(
        &results[1].result,
        Err(error) if error.message == "deliberate failure"
    ));
    assert_eq!(output.circuit.time(), 0.0);
}

#[test]
fn rejects_duplicate_language_test_names() {
    let source = r#"
        test(name: "duplicate", body: () -> true);
        test(name: "duplicate", body: () -> true);
    "#;
    assert!(matches!(
        evaluate(&Sources::single(source)),
        Err(RunError::Evaluation(error)) if error.message == "duplicate test name `duplicate`"
    ));
}

#[test]
fn standard_library_examples_are_executable_tests() {
    let examples = [
        (
            "voltage_divider.etch",
            include_str!("../../examples/voltage_divider.etch"),
        ),
        (
            "rc_response.etch",
            include_str!("../../examples/rc_response.etch"),
        ),
        ("op_amp.etch", include_str!("../../examples/op_amp.etch")),
        (
            "logic_gates.etch",
            include_str!("../../examples/logic_gates.etch"),
        ),
        (
            "d_flip_flop.etch",
            include_str!("../../examples/d_flip_flop.etch"),
        ),
        (
            "sectioned_system.etch",
            include_str!("../../examples/sectioned_system.etch"),
        ),
        (
            "complex_mixed_signal.etch",
            include_str!("../../examples/complex_mixed_signal.etch"),
        ),
    ];

    for (filename, source) in examples {
        let output = evaluate(&Sources::single(source))
            .unwrap_or_else(|error| panic!("{filename} failed to evaluate: {error:?}"));
        let results = output.run_tests();
        assert!(
            !results.is_empty(),
            "{filename} does not register any tests"
        );
        for result in results {
            assert!(
                result.result.is_ok(),
                "{}: test `{}` failed: {:?}",
                filename,
                result.name,
                result.result.err()
            );
        }
    }
}

#[test]
fn samples_registered_displays() {
    let source = include_str!("../../examples/rc_response.etch");
    let output = evaluate(&Sources::single(source)).unwrap();
    assert_eq!(output.circuit.display_count(), 1);
    assert_eq!(output.circuit.time(), 0.0);

    let displays = output.run_displays();
    assert_eq!(displays.len(), 1);
    assert_eq!(displays[0].name, "RC transient response");
    let graph = displays[0].result.as_ref().unwrap();
    assert_eq!(graph.traces.len(), 2);
    assert!(graph.traces.iter().all(|trace| trace.samples.len() == 21));
    assert_eq!(output.circuit.time(), 0.0);
}

#[test]
fn reports_display_trace_errors_without_mutating_the_circuit() {
    let source = r#"
        display(
            name: "invalid trace",
            steps: 2,
            delta_time: 0.001,
            traces: { status: () -> true },
        );
    "#;
    let output = evaluate(&Sources::single(source)).unwrap();
    let displays = output.run_displays();
    assert!(matches!(
        &displays[0].result,
        Err(error) if error.message == "display trace `status` must return a number"
    ));
    assert_eq!(output.circuit.time(), 0.0);
}

#[test]
fn rejects_unexpected_native_arguments() {
    for expression in [
        "voltage(node: ground, typo: 1)",
        "conductance(between: [ground, ground], value: 1, typo: 1)",
        "current(between: [ground, ground], value: 1, typo: 1)",
        "fix_voltage(node: ground, value: 1, typo: 1)",
        "drive_voltage(node: ground, value: 1, conductance: 1, typo: 1)",
    ] {
        let error = evaluate(&Sources::single(expression)).unwrap_err();
        assert!(
            matches!(error, RunError::Evaluation(ref error) if error.message.contains("unexpected argument `typo`")),
            "{error:?}"
        );
    }
}

#[test]
fn rejects_invalid_simulation_time_without_mutation() {
    let circuit = crate::Circuit::default();
    for delta in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(circuit.simulate(1, delta).is_err());
        assert_eq!(circuit.time(), 0.0);
    }
}

#[test]
fn equality_preserves_units_and_circuit_identity() {
    assert_ne!(Value::Number(1.0), Value::Quantity(1.0, "V".into()));
    let first = evaluate(&Sources::single("ground")).unwrap();
    let second = evaluate(&Sources::single("ground")).unwrap();
    assert_ne!(first.values, second.values);
    assert_eq!(first.values, first.values.clone());
}

#[test]
fn evaluates_shadowing_and_chained_access() {
    assert_eq!(run("let x = 1; let x = x + 1; x"), vec![Value::Number(2.0)]);
    assert_eq!(
        run("let make = () -> { value: () -> { answer: 42 } }; make().value().answer"),
        vec![Value::Number(42.0)]
    );
}

#[test]
fn detects_conflicting_fixed_voltages() {
    let output = evaluate(&Sources::single("let make = () -> use_equation(equation: () -> fix_voltage(node: ground, value: 5)); make()")).unwrap();
    assert!(output.circuit.simulate(1, 0.001).is_err());
}

#[test]
fn quantities_and_string_escapes_survive_evaluation() {
    assert_eq!(run("2mV + 3mV"), vec![Value::Quantity(0.005, "V".into())]);
    assert_eq!(
        run(r#""a\"b\\c\n\t""#),
        vec![Value::String("a\"b\\c\n\t".into())]
    );
}
#[test]
fn check_validates_imports_without_running_code() {
    let sources = Sources {
        main: "main.etch".into(),
        files: BTreeMap::from([
            (
                "main.etch".into(),
                "from \"./sub/../module.etch\" import { value }; assert(condition: false); value"
                    .into(),
            ),
            (
                "module.etch".into(),
                "export let value = 42; assert(condition: false)".into(),
            ),
        ]),
    };
    assert!(crate::check(&sources).is_ok());
    assert!(evaluate(&sources).is_err());
    let mut missing = sources;
    missing.files.insert(
        "main.etch".into(),
        "from \"module.etch\" import { absent }; absent".into(),
    );
    assert!(
        crate::check(&missing)
            .unwrap_err()
            .to_string()
            .contains("does not export `absent`")
    );
    missing.files.insert(
        "main.etch".into(),
        "from \"module.etch\" import { value }; value + true".into(),
    );
    assert!(
        crate::check(&missing)
            .unwrap_err()
            .to_string()
            .contains("invalid operands")
    );
}
#[test]
fn releasing_outputs_reclaims_circuits_and_recursive_closures() {
    for _ in 0..20 {
        let output = evaluate(&Sources::single("let fact = (n: Number) -> match n { 0 -> 1, let n -> n * fact(n: n - 1) }; let make = () -> use_equation(equation: () -> voltage(node: ground)); make(); fact")).unwrap();
        let circuit = std::rc::Rc::downgrade(&output.circuit.inner);
        let Value::Lambda(lambda) = &output.values[0] else {
            panic!("expected closure")
        };
        let scope = std::rc::Rc::downgrade(&lambda.scope.values);
        drop(output);
        assert!(circuit.upgrade().is_none());
        assert!(scope.upgrade().is_none());
    }
    assert_eq!(
        run("let make = (x: Number) -> (y: Number) -> x + y; let add = make(x: 2); add(y: 3)"),
        vec![Value::Number(5.0)]
    );
}
#[test]
fn foreign_circuit_nodes_are_rejected() {
    let a = crate::Circuit::default();
    let b = crate::Circuit::default();
    let first = a.node();
    let second = b.node();
    assert!(a.voltage(&second).is_err());
    assert!(first.connect(&second).is_err());
    assert!(first.connections().unwrap().is_empty());
}
#[test]
fn failed_simulation_rolls_back_and_floating_circuits_fail() {
    for source in [
        "let make = () -> { let a = use_node(); let b = use_node(); use_equation(equation: () -> conductance(between: [a,b], value: 1)) }; make()",
        "let make = () -> { let a = use_node(); use_equation(equation: () -> drive_voltage(node: a, value: match voltage(node: a) > 0.5 { true -> 0, false -> 1 }, conductance: 1)) }; make()",
        "let make = () -> use_equation(equation: () -> assert(condition: time() < 1)); make()",
    ] {
        let output = evaluate(&Sources::single(source)).unwrap();
        let before = output.circuit.node_voltages();
        assert!(output.circuit.simulate(2, 1.0).is_err());
        assert_eq!(output.circuit.node_voltages(), before);
        assert_eq!(output.circuit.time(), 0.0);
    }
}
#[test]
fn solves_a_resistor_ladder_against_its_analytic_solution() {
    let circuit = crate::Circuit::default();
    let low = circuit.ground();
    let nodes: Vec<_> = (0..10).map(|_| circuit.node()).collect();
    {
        let mut state = circuit.inner.borrow_mut();
        state.fixed.insert(low.id, 0.0);
        state.fixed.insert(nodes[9].id, 10.0);
        state.conductances.push((low.id, nodes[0].id, 1.0));
        for pair in nodes.windows(2) {
            state.conductances.push((pair[0].id, pair[1].id, 1.0));
        }
    }
    circuit.solve_iteration().unwrap();
    for (index, node) in nodes.iter().enumerate() {
        assert!((circuit.voltage(node).unwrap() - (index + 1) as f64).abs() < 1e-10);
    }
}

#[test]
fn aliasing_a_recursive_closure_keeps_its_self_binding() {
    assert_eq!(
        run(
            "let f = (n: Number) -> match n { 0 -> 1, let n -> n * f(n: n - 1) }; let alias = f; alias(n: 5)"
        ),
        vec![Value::Number(120.0)]
    );
}
