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
fn evaluates_lambdas_and_returns() {
    assert_eq!(
        run("let double = (x: Number) -> { return x * 2 }; double(x: 4)"),
        vec![Value::Number(8.0)]
    );
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
    assert_eq!(node.connections().len(), 1);
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
        from "std/sources.txt" import { VoltageSource, Ground };
        from "std/passive.txt" import { Resistor };
        from "std/analog.txt" import { OpAmp, Led };
        from "std/digital.txt" import { NotGate, DFlipFlop };

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
    assert_eq!(output.circuit.node_count(), 6);
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
    assert_eq!(output.circuit.voltage(high), 5.0);
    assert!((output.circuit.voltage(middle) - 2.5).abs() < 1e-6);
    assert_eq!(output.circuit.voltage(low), 0.0);
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
    assert!((output.circuit.voltage(analog) - 1.0).abs() < 1e-6);
    assert_eq!(output.circuit.voltage(digital), 0.0);
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
    assert!((output.circuit.voltage(node) - 1.0 / 11.0).abs() < 1e-6);
    output.circuit.simulate(1, 0.001).unwrap();
    assert!((output.circuit.voltage(node) - 1.0 / 121.0).abs() < 1e-6);
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
    assert_eq!(output.circuit.voltage(q), 0.0);
    output.circuit.simulate(1, 0.001).unwrap();
    assert_eq!(output.circuit.voltage(q), 5.0);
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
