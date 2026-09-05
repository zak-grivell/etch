use circuit_ir::Component;
use std::collections::BTreeMap;

use super::{
    Hitbox, Point, STYLE_AND_SYMBOLS, component_size, component_value, generic_port_offset,
    segment_clear, svg_port,
};

#[test]
fn formats_component_values_with_engineering_units() {
    assert_eq!(component_value("1000 Ω"), "1 kΩ");
    assert_eq!(component_value("0.000001 F"), "1 µF");
    assert_eq!(component_value("0.0022 H"), "2.2 mH");
    assert_eq!(component_value("5 V"), "5 V");
    assert_eq!(component_value("0.00002 A"), "20 µA");
    assert_eq!(component_value("12"), "12");
    assert_eq!(component_value("variable"), "variable");
}

#[test]
fn reads_named_port_locations_from_svg_metadata() {
    assert_eq!(
        svg_port(STYLE_AND_SYMBOLS, "symbol-op-amp", "positive"),
        Some((-40.0, -14.0))
    );
    assert_eq!(
        svg_port(STYLE_AND_SYMBOLS, "symbol-ground", "node"),
        Some((0.0, -25.0))
    );
    assert_eq!(
        svg_port(
            r#"<g data-ports="input:-32,-8;output:36,4"></g>"#,
            "",
            "output"
        ),
        Some((36.0, 4.0))
    );
}

#[test]
fn routing_segments_treat_interior_hitboxes_as_hard_obstacles() {
    let obstacle = Hitbox {
        left: 40.0,
        right: 80.0,
        top: 30.0,
        bottom: 70.0,
    };
    assert!(!segment_clear(
        Point { x: 10.0, y: 50.0 },
        Point { x: 100.0, y: 50.0 },
        &[obstacle]
    ));
    assert!(!segment_clear(
        Point { x: 60.0, y: 10.0 },
        Point { x: 60.0, y: 90.0 },
        &[obstacle]
    ));
    assert!(segment_clear(
        Point { x: 10.0, y: 20.0 },
        Point { x: 100.0, y: 20.0 },
        &[obstacle]
    ));
}

#[test]
fn generic_ic_geometry_scales_and_splits_pins_across_both_sides() {
    let mut component = Component {
        kind: "large-controller".into(),
        label: None,
        value: None,
        ports: BTreeMap::new(),
        section: None,
        svg: None,
        kicad: None,
    };
    for index in 0..20 {
        component.ports.insert(format!("pin_{index}"), index);
    }
    let (width, height) = component_size(&component);
    assert!(width >= 150.0);
    assert!(height >= 20.0 * 10.0);
    assert!(generic_port_offset(&component, 0, 20).0 < 0.0);
    assert!(generic_port_offset(&component, 10, 20).0 > 0.0);
}

#[test]
fn preserves_two_ports_on_the_same_electrical_node() {
    let component = Component {
        kind: "resistor".into(),
        label: None,
        value: None,
        section: None,
        svg: None,
        kicad: None,
        ports: BTreeMap::from([("a".into(), 1), ("b".into(), 1)]),
    };
    let ports = super::port_points(
        &component,
        Point { x: 0.0, y: 0.0 },
        super::Orientation::Horizontal,
    );
    assert_eq!(ports.0.len(), 2);
    assert_ne!(ports.0["a"].1.x, ports.0["b"].1.x);
}
#[test]
fn crossing_nets_fall_back_without_partial_wires() {
    let mut svg = String::new();
    let result = super::draw_orthogonal_net(
        &mut svg,
        &[Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }],
        &[],
        &circuit_ir::CircuitDesign::default(),
        &BTreeMap::new(),
        &[(Point { x: 50.0, y: -100.0 }, Point { x: 50.0, y: 100.0 })],
    );
    assert!(result.is_err());
    assert!(svg.is_empty());
}
