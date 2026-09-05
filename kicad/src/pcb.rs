use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use circuit_ir::{CircuitDesign, sexpr::SExpr};

use crate::common::*;

pub fn pcb(circuit: &CircuitDesign) -> Result<String, String> {
    ensure_links(circuit)?;
    let config = circuit
        .pcb
        .as_ref()
        .ok_or_else(|| "KiCad PCB export requires pcb_config(...)".to_owned())?;
    let layout = pcb::layout(circuit)?;
    if layout.unrouted_connections > 0 {
        return Err(format!(
            "PCB export has {} unrouted connections; increase board space or revise placement constraints",
            layout.unrouted_connections
        ));
    }
    let physical = layout
        .components
        .iter()
        .filter(|placed| {
            !circuit.components[placed.component_index]
                .kicad
                .as_ref()
                .unwrap()
                .footprint
                .is_empty()
        })
        .collect::<Vec<_>>();
    if physical.is_empty() {
        return Err("KiCad PCB export requires at least one component with a footprint".into());
    }
    let roots = circuit.electrical_roots();
    let names = circuit.net_names();
    let mut roots_used = BTreeSet::new();
    for placed in &physical {
        let component = &circuit.components[placed.component_index];
        for node in component.ports.values() {
            roots_used.insert(roots.get(node).copied().unwrap_or(*node));
        }
    }
    let net_ids = roots_used
        .iter()
        .enumerate()
        .map(|(index, root)| (*root, index + 1))
        .collect::<BTreeMap<_, _>>();
    let version = physical
        .iter()
        .filter_map(|placed| {
            placed
                .footprint
                .definition
                .field("version")
                .and_then(|v| v.value(1))
                .and_then(|v| v.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(20240108);
    let mut out = String::from(
        "(kicad_pcb (version 20240108) (generator etch)\n  (general (thickness 1.6))\n  (paper \"A4\")\n  (layers\n    (0 \"F.Cu\" signal)\n",
    );
    out = out.replace("20240108", &version.to_string());
    for layer in 1..config.layers.saturating_sub(1) {
        writeln!(out, "    ({} \"In{layer}.Cu\" signal)", layer * 2).unwrap();
    }
    out.push_str("    (31 \"B.Cu\" signal)\n    (36 \"B.SilkS\" user \"b.silkscreen\")\n    (37 \"F.SilkS\" user \"f.silkscreen\")\n    (44 \"Edge.Cuts\" user)\n  )\n  (setup (pad_to_mask_clearance 0))\n  (net 0 \"\")\n");
    for (root, id) in &net_ids {
        writeln!(out, "  (net {id} \"{}\")", esc(names.get(root).unwrap())).unwrap();
    }
    let references = references(circuit)?;
    let map = |point: pcb::Point| Point {
        x: 100.0 + config.width / 2.0 + point.x,
        y: 100.0 + config.height / 2.0 - point.y,
    };
    for (index, placed) in physical.iter().enumerate() {
        let component = &circuit.components[placed.component_index];
        let center = map(placed.center);
        let link = component.kicad.as_ref().unwrap();
        let reference = &references[placed.component_index];
        let mut definition = placed.footprint.definition.clone();
        definition.items_mut()[1] = SExpr::string(&link.footprint);
        definition.remove_recursive("uuid");
        definition.remove_recursive("tstamp");
        for name in ["version", "generator", "generator_version"] {
            definition.remove(name);
        }
        definition.set(SExpr::list(
            "at",
            [
                SExpr::atom(center.x),
                SExpr::atom(center.y),
                SExpr::atom(placed.rotation),
            ],
        ));
        let at = definition.field("at").unwrap().clone();
        definition.remove("at");
        definition.items_mut().insert(2, at);
        definition.set(SExpr::list("uuid", [SExpr::string(uuid(index + 1, 0))]));
        for item in definition.items_mut() {
            if item.tag() == Some("property") || item.tag() == Some("fp_text") {
                match item.value(1) {
                    Some("Reference" | "reference") => {
                        item.items_mut()[2] = SExpr::string(reference)
                    }
                    Some("Value" | "value") => {
                        item.items_mut()[2] =
                            SExpr::string(component.value.as_deref().unwrap_or(&component.kind))
                    }
                    _ => {}
                }
            }
            if item.tag() == Some("pad") {
                let number = item.value(1).unwrap_or("");
                let node = placed
                    .pads
                    .iter()
                    .find(|pad| pad.number == number)
                    .and_then(|pad| pad.node);
                item.remove("net");
                if let Some(node) = node {
                    let root = roots[&node];
                    item.set(SExpr::list(
                        "net",
                        [SExpr::atom(net_ids[&root]), SExpr::string(&names[&root])],
                    ));
                }
            }
            if matches!(item.tag(), Some("pad" | "property" | "fp_text"))
                && let Some(at) = item.field("at").cloned()
            {
                let angle = at.value(3).map_or(Ok(0.0), |_| at.number(3))? + placed.rotation as f64;
                item.set(SExpr::list(
                    "at",
                    [
                        SExpr::atom(at.number(1)?),
                        SExpr::atom(at.number(2)?),
                        SExpr::atom(angle),
                    ],
                ));
            }
        }
        writeln!(out, "  {definition}").unwrap();
    }
    for trace in &layout.traces {
        let Some(net) = net_ids.get(&trace.root).copied() else {
            continue;
        };
        let layer = copper_layer_name(trace.layer, config.layers);
        let points = trace.points.iter().copied().map(map).collect::<Vec<_>>();
        for pair in points.windows(2) {
            segment(
                &mut out,
                pair[0],
                pair[1],
                config.min_trace_width,
                &layer,
                net,
            );
        }
    }
    let x0 = 100.0;
    let y0 = 100.0;
    let x1 = x0 + config.width;
    let y1 = y0 + config.height;
    for (a, b) in [
        (Point { x: x0, y: y0 }, Point { x: x1, y: y0 }),
        (Point { x: x1, y: y0 }, Point { x: x1, y: y1 }),
        (Point { x: x1, y: y1 }, Point { x: x0, y: y1 }),
        (Point { x: x0, y: y1 }, Point { x: x0, y: y0 }),
    ] {
        writeln!(out, "  (gr_line (start {} {}) (end {} {}) (stroke (width 0.05) (type default)) (layer \"Edge.Cuts\"))", a.x, a.y, b.x, b.y).unwrap();
    }
    out.push_str(")\n");
    Ok(out)
}

fn copper_layer_name(layer: usize, count: usize) -> String {
    if layer == 0 || count == 1 {
        "F.Cu".into()
    } else if layer + 1 >= count {
        "B.Cu".into()
    } else {
        format!("In{layer}.Cu")
    }
}

fn segment(out: &mut String, a: Point, b: Point, width: f64, layer: &str, net: usize) {
    if (a.x - b.x).abs() < f64::EPSILON && (a.y - b.y).abs() < f64::EPSILON {
        return;
    }
    writeln!(
        out,
        "  (segment (start {} {}) (end {} {}) (width {width}) (layer \"{layer}\") (net {net}))",
        a.x, a.y, b.x, b.y
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_configured_copper_stack_to_kicad_layers() {
        assert_eq!(copper_layer_name(0, 4), "F.Cu");
        assert_eq!(copper_layer_name(1, 4), "In1.Cu");
        assert_eq!(copper_layer_name(2, 4), "In2.Cu");
        assert_eq!(copper_layer_name(3, 4), "B.Cu");
    }
}
