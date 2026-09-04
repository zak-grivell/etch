use std::collections::BTreeMap;
use std::fmt::Write;

use circuit_ir::{CircuitDesign, Component};

use crate::common::*;
use crate::symbol_library::{self, LibrarySymbol, Pin};

pub fn schematic(circuit: &CircuitDesign) -> Result<String, String> {
    ensure_links(circuit)?;
    let layout = schematic::layout(circuit);
    let roots = circuit.electrical_roots();
    let names = circuit.net_names();
    let root_uuid = uuid(0, 0);
    let mut out = format!(
        "(kicad_sch (version 20250114) (generator \"etch\") (generator_version \"0.1\")\n  (uuid \"{root_uuid}\")\n  (paper \"A1\")\n  (lib_symbols\n"
    );
    let mut libraries = BTreeMap::<String, (&Component, String)>::new();
    for component in &circuit.components {
        let link = component.kicad.as_ref().unwrap();
        libraries.entry(link.symbol.clone()).or_insert_with(|| {
            let base = link
                .symbol
                .rsplit(':')
                .next()
                .unwrap_or(&link.symbol)
                .to_owned();
            (component, base)
        });
    }
    let mut loaded_symbols = BTreeMap::new();
    for (library, (component, base)) in &libraries {
        let link = component.kicad.as_ref().unwrap();
        if let Some(symbol) = symbol_library::load(library)? {
            out.push_str("    ");
            out.push_str(&symbol.definition.replace('\n', "\n    "));
            out.push('\n');
            loaded_symbols.insert(library.clone(), symbol);
            continue;
        }
        writeln!(out, "    (symbol \"{}\"", esc(library)).unwrap();
        out.push_str("      (pin_names (offset 1.016))\n      (exclude_from_sim no) (in_bom yes) (on_board yes)\n");
        property(
            &mut out,
            "Reference",
            &reference_prefix(library),
            0.0,
            -5.0,
            false,
        );
        property(&mut out, "Value", base, 0.0, 5.0, false);
        property(&mut out, "Footprint", &link.footprint, 0.0, 0.0, true);
        property(&mut out, "Datasheet", "~", 0.0, 0.0, true);
        property(
            &mut out,
            "Description",
            "Etch linked component",
            0.0,
            0.0,
            true,
        );
        writeln!(out, "      (symbol \"{}_1_1\"", esc(base)).unwrap();
        out.push_str("        (rectangle (start -2.54 -3.81) (end 2.54 3.81) (stroke (width 0.254) (type default)) (fill (type background)))\n");
        for (index, (port, pin)) in link.pins.iter().enumerate() {
            let y = pin_y(index, link.pins.len());
            writeln!(out, "        (pin passive line (at -5.08 {y} 0) (length 2.54) (name \"{}\" (effects (font (size 1.27 1.27)))) (number \"{}\" (effects (font (size 1.27 1.27)))))", esc(port), esc(pin)).unwrap();
        }
        out.push_str("      )\n      (embedded_fonts no)\n    )\n");
        loaded_symbols.insert(library.clone(), generic_symbol(link));
    }
    out.push_str("  )\n");
    let mut references = BTreeMap::<String, usize>::new();
    let mut net_points = BTreeMap::<u64, Vec<Point>>::new();
    const PAGE_MARGIN_MM: f64 = 20.0;
    const A1_WIDTH_MM: f64 = 841.0;
    const A1_HEIGHT_MM: f64 = 594.0;
    const DEFAULT_SVG_UNITS_PER_MM: f64 = 4.0;
    let units_per_mm = DEFAULT_SVG_UNITS_PER_MM
        .max(layout.width / (A1_WIDTH_MM - PAGE_MARGIN_MM * 2.0))
        .max(layout.height / (A1_HEIGHT_MM - PAGE_MARGIN_MM * 2.0));
    for (index, component) in circuit.components.iter().enumerate() {
        let link = component.kicad.as_ref().unwrap();
        let loaded = &loaded_symbols[&link.symbol];
        let prefix = reference_prefix(&link.symbol);
        let number = references.entry(prefix.clone()).or_default();
        *number += 1;
        let reference = format!("{prefix}{number}");
        let x = PAGE_MARGIN_MM + layout.components[index].x / units_per_mm;
        let y = PAGE_MARGIN_MM + layout.components[index].y / units_per_mm;
        let value = component.value.as_deref().unwrap_or(&component.kind);
        writeln!(out, "  (symbol (lib_id \"{}\") (at {x} {y} 0) (unit 1) (exclude_from_sim no) (in_bom yes) (on_board yes) (dnp no) (uuid \"{}\")", esc(&link.symbol), uuid(index + 1, 0)).unwrap();
        instance_property(&mut out, "Reference", &reference, x, y - 5.0, false);
        instance_property(&mut out, "Value", value, x, y + 5.0, false);
        instance_property(&mut out, "Footprint", &link.footprint, x, y, true);
        instance_property(&mut out, "Datasheet", "~", x, y, true);
        instance_property(&mut out, "Description", "Etch linked component", x, y, true);
        for (port, node) in &component.ports {
            let root = roots.get(node).copied().unwrap_or(*node);
            let pin_number = &link.pins[port];
            let pin = loaded.pins.get(pin_number).ok_or_else(|| {
                format!(
                    "KiCad symbol `{}` does not contain mapped pin `{pin_number}` for port `{port}`",
                    link.symbol
                )
            })?;
            net_points.entry(root).or_default().push(Point {
                x: x + pin.x,
                y: y + pin.y,
            });
        }
        for (pin_index, pin_number) in loaded.pins.keys().enumerate() {
            writeln!(
                out,
                "    (pin \"{}\" (uuid \"{}\"))",
                esc(pin_number),
                uuid(index + 1, pin_index + 1)
            )
            .unwrap();
        }
        writeln!(out, "    (instances (project \"etch\" (path \"/{root_uuid}\" (reference \"{}\") (unit 1))))\n  )", esc(&reference)).unwrap();
    }
    let mut wire_id = 1usize;
    for (root, points) in net_points.iter().filter(|(_, points)| points.len() > 1) {
        let mut previous = points[0];
        for target in points.iter().copied().skip(1) {
            let bend = Point {
                x: target.x,
                y: previous.y,
            };
            sch_wire(&mut out, previous, bend, wire_id);
            wire_id += 1;
            sch_wire(&mut out, bend, target, wire_id);
            wire_id += 1;
            previous = target;
        }
        writeln!(out, "  (label \"{}\" (at {} {} 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid \"{}\"))", esc(names.get(root).unwrap()), points[0].x, points[0].y, uuid(9000, wire_id)).unwrap();
        wire_id += 1;
    }
    out.push_str("  (sheet_instances (path \"/\" (page \"1\")))\n  (embedded_fonts no)\n)\n");
    Ok(out)
}

fn generic_symbol(link: &circuit_ir::KicadLink) -> LibrarySymbol {
    let pins = link
        .pins
        .iter()
        .enumerate()
        .map(|(index, (_, number))| {
            (
                number.clone(),
                Pin {
                    x: -5.08,
                    y: pin_y(index, link.pins.len()),
                },
            )
        })
        .collect();
    LibrarySymbol {
        definition: String::new(),
        pins,
    }
}

fn property(out: &mut String, name: &str, value: &str, x: f64, y: f64, hide: bool) {
    writeln!(
        out,
        "      (property \"{}\" \"{}\" (at {x} {y} 0) (effects (font (size 1.27 1.27)){}))",
        esc(name),
        esc(value),
        if hide { " (hide yes)" } else { "" }
    )
    .unwrap();
}

fn instance_property(out: &mut String, name: &str, value: &str, x: f64, y: f64, hide: bool) {
    writeln!(
        out,
        "    (property \"{}\" \"{}\" (at {x} {y} 0) (effects (font (size 1.27 1.27)){}))",
        esc(name),
        esc(value),
        if hide { " (hide yes)" } else { "" }
    )
    .unwrap();
}

fn pin_y(index: usize, count: usize) -> f64 {
    (index as f64 - count.saturating_sub(1) as f64 / 2.0) * 2.54
}

fn sch_wire(out: &mut String, a: Point, b: Point, id: usize) {
    if (a.x - b.x).abs() < f64::EPSILON && (a.y - b.y).abs() < f64::EPSILON {
        return;
    }
    writeln!(
        out,
        "  (wire (pts (xy {} {}) (xy {} {})) (stroke (width 0) (type default)) (uuid \"{}\"))",
        a.x,
        a.y,
        b.x,
        b.y,
        uuid(8000, id)
    )
    .unwrap();
}
