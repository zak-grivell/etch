use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use circuit_ir::{CircuitDesign, Component, PcbConfig};

use crate::common::*;

pub fn pcb(circuit: &CircuitDesign) -> Result<String, String> {
    ensure_links(circuit)?;
    let config = circuit
        .pcb
        .as_ref()
        .ok_or_else(|| "KiCad PCB export requires pcb_config(...)".to_owned())?;
    let physical = circuit
        .components
        .iter()
        .filter(|component| !component.kicad.as_ref().unwrap().footprint.is_empty())
        .collect::<Vec<_>>();
    if physical.is_empty() {
        return Err("KiCad PCB export requires at least one component with a footprint".into());
    }
    let roots = circuit.electrical_roots();
    let names = circuit.net_names();
    let positions = place(&physical, config)?;
    let mut roots_used = BTreeSet::new();
    for component in &physical {
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
        "(kicad_pcb (version 20240108) (generator etch)\n  (general (thickness 1.6))\n  (paper \"A4\")\n  (layers\n    (0 \"F.Cu\" signal)\n    (31 \"B.Cu\" signal)\n    (36 \"B.SilkS\" user \"b.silkscreen\")\n    (37 \"F.SilkS\" user \"f.silkscreen\")\n    (44 \"Edge.Cuts\" user)\n  )\n  (setup (pad_to_mask_clearance 0))\n  (net 0 \"\")\n",
    );
    for (root, id) in &net_ids {
        writeln!(out, "  (net {id} \"{}\")", esc(names.get(root).unwrap())).unwrap();
    }
    let mut refs = BTreeMap::<String, usize>::new();
    let mut net_points = BTreeMap::<u64, Vec<Point>>::new();
    for (index, (component, center)) in physical.iter().zip(&positions).enumerate() {
        let link = component.kicad.as_ref().unwrap();
        let prefix = reference_prefix(&link.symbol);
        let count = refs.entry(prefix.clone()).or_default();
        *count += 1;
        let reference = format!("{prefix}{count}");
        writeln!(out, "  (footprint \"{}\"", esc(&link.footprint)).unwrap();
        writeln!(out, "    (layer \"F.Cu\") (at {} {})", center.x, center.y).unwrap();
        writeln!(out, "    (uuid \"{}\")", uuid(index + 1, 0)).unwrap();
        writeln!(out, "    (property \"Reference\" \"{}\" (at 0 -2 0) (layer \"F.SilkS\") (effects (font (size 1 1) (thickness 0.15))))", esc(&reference)).unwrap();
        writeln!(out, "    (property \"Value\" \"{}\" (at 0 2 0) (layer \"F.Fab\") hide (effects (font (size 1 1) (thickness 0.15))))", esc(component.value.as_deref().unwrap_or(&component.kind))).unwrap();
        let count = component.ports.len();
        for (pad_index, (port, node)) in component.ports.iter().enumerate() {
            let pad_x = (pad_index as f64 - (count.saturating_sub(1) as f64 / 2.0)) * 1.8;
            let root = roots.get(node).copied().unwrap_or(*node);
            let point = Point {
                x: center.x + pad_x,
                y: center.y,
            };
            net_points.entry(root).or_default().push(point);
            writeln!(out, "    (pad \"{}\" thru_hole circle (at {pad_x} 0) (size 1.5 1.5) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net {} \"{}\") (uuid \"{}\"))", esc(&link.pins[port]), net_ids[&root], esc(names.get(&root).unwrap()), uuid(index + 1, pad_index + 1)).unwrap();
        }
        out.push_str("  )\n");
    }
    for (route_index, (root, points)) in net_points
        .iter()
        .filter(|(_, points)| points.len() > 1)
        .enumerate()
    {
        let layer = if config.layers > 1 && route_index % 2 == 1 {
            "B.Cu"
        } else {
            "F.Cu"
        };
        let mut previous = points[0];
        for target in points.iter().copied().skip(1) {
            let bend = Point {
                x: target.x,
                y: previous.y,
            };
            segment(
                &mut out,
                previous,
                bend,
                config.min_trace_width,
                layer,
                net_ids[root],
            );
            segment(
                &mut out,
                bend,
                target,
                config.min_trace_width,
                layer,
                net_ids[root],
            );
            previous = target;
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

fn place(components: &[&Component], config: &PcbConfig) -> Result<Vec<Point>, String> {
    let columns = ((config.width - 6.0) / 8.0).floor().max(1.0) as usize;
    let mut points = Vec::new();
    for index in 0..components.len() {
        let point = Point {
            x: 104.0 + (index % columns) as f64 * 8.0,
            y: 104.0 + (index / columns) as f64 * 7.0,
        };
        if point.y > 100.0 + config.height - 3.0 {
            return Err("components do not fit within the configured PCB".into());
        }
        points.push(point);
    }
    Ok(points)
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
