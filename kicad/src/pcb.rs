use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use circuit_ir::CircuitDesign;

use crate::common::*;

pub fn pcb(circuit: &CircuitDesign) -> Result<String, String> {
    ensure_links(circuit)?;
    let config = circuit
        .pcb
        .as_ref()
        .ok_or_else(|| "KiCad PCB export requires pcb_config(...)".to_owned())?;
    let layout = pcb::layout(circuit)?;
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
    let mut out = String::from(
        "(kicad_pcb (version 20240108) (generator etch)\n  (general (thickness 1.6))\n  (paper \"A4\")\n  (layers\n    (0 \"F.Cu\" signal)\n",
    );
    for layer in 1..config.layers.saturating_sub(1) {
        writeln!(out, "    ({} \"In{layer}.Cu\" signal)", layer * 2).unwrap();
    }
    out.push_str("    (31 \"B.Cu\" signal)\n    (36 \"B.SilkS\" user \"b.silkscreen\")\n    (37 \"F.SilkS\" user \"f.silkscreen\")\n    (44 \"Edge.Cuts\" user)\n  )\n  (setup (pad_to_mask_clearance 0))\n  (net 0 \"\")\n");
    for (root, id) in &net_ids {
        writeln!(out, "  (net {id} \"{}\")", esc(names.get(root).unwrap())).unwrap();
    }
    let mut refs = BTreeMap::<String, usize>::new();
    let map = |point: pcb::Point| Point {
        x: 100.0 + config.width / 2.0 + point.x,
        y: 100.0 + config.height / 2.0 - point.y,
    };
    for (index, placed) in physical.iter().enumerate() {
        let component = &circuit.components[placed.component_index];
        let center = map(placed.center);
        let link = component.kicad.as_ref().unwrap();
        let prefix = reference_prefix(&link.symbol);
        let count = refs.entry(prefix.clone()).or_default();
        *count += 1;
        let reference = format!("{prefix}{count}");
        writeln!(out, "  (footprint \"{}\"", esc(&link.footprint)).unwrap();
        // Pad offsets below already include the packer's selected rotation.
        writeln!(out, "    (layer \"F.Cu\") (at {} {})", center.x, center.y).unwrap();
        writeln!(out, "    (uuid \"{}\")", uuid(index + 1, 0)).unwrap();
        writeln!(out, "    (property \"Reference\" \"{}\" (at 0 -2 0) (layer \"F.SilkS\") (effects (font (size 1 1) (thickness 0.15))))", esc(&reference)).unwrap();
        writeln!(out, "    (property \"Value\" \"{}\" (at 0 2 0) (layer \"F.Fab\") hide (effects (font (size 1 1) (thickness 0.15))))", esc(component.value.as_deref().unwrap_or(&component.kind))).unwrap();
        for (pad_index, (port, node)) in component.ports.iter().enumerate() {
            let root = roots.get(node).copied().unwrap_or(*node);
            let pad = map(placed.pads[node]);
            let pad_x = pad.x - center.x;
            let pad_y = pad.y - center.y;
            writeln!(out, "    (pad \"{}\" thru_hole circle (at {pad_x} {pad_y}) (size 1.2 1.2) (drill 0.6) (layers \"*.Cu\" \"*.Mask\") (net {} \"{}\") (uuid \"{}\"))", esc(&link.pins[port]), net_ids[&root], esc(names.get(&root).unwrap()), uuid(index + 1, pad_index + 1)).unwrap();
        }
        out.push_str("  )\n");
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

fn ensure_links(circuit: &CircuitDesign) -> Result<(), String> {
    circuit
        .components
        .iter()
        .find(|component| component.kicad.is_none())
        .map_or(Ok(()), |component| {
            Err(format!(
                "component kind `{}` has no KiCad link; add kicad: {{ symbol, footprint, pins }} to use_symbol",
                component.kind
            ))
        })
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
