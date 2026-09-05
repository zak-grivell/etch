use std::collections::BTreeMap;
use std::fmt::Write;

use circuit_ir::{CircuitDesign, Component};

use crate::common::*;
use crate::symbol_library::{self, LibrarySymbol, Pin};

#[derive(Clone, Copy)]
struct SymbolPosition {
    x: f64,
    y: f64,
    rotation: i32,
}

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
    for component in &circuit.components {
        let link = component.kicad.as_ref().unwrap();
        for (port, number) in &link.pins {
            if !loaded_symbols[&link.symbol].pins.contains_key(number) {
                return Err(format!(
                    "KiCad symbol `{}` does not contain mapped pin `{number}` for port `{port}` in unit 1 (mapping other units is unsupported)",
                    link.symbol
                ));
            }
        }
    }
    let positions = place_symbols(circuit, &loaded_symbols, &layout);
    let references = references(circuit)?;
    let mut net_points = BTreeMap::<u64, Vec<NetPoint>>::new();
    for (index, component) in circuit.components.iter().enumerate() {
        let link = component.kicad.as_ref().unwrap();
        let loaded = &loaded_symbols[&link.symbol];
        let reference = &references[index];
        // KiCad's editor and symbol libraries are built around a 50 mil
        // (1.27 mm) grid.  Snapping the symbol origins keeps every pin and
        // label editable without the tiny off-grid wire fragments produced by
        // arbitrary SVG-to-page scaling.
        let SymbolPosition { x, y, rotation } = positions[index];
        let value = component.value.as_deref().unwrap_or(&component.kind);
        writeln!(out, "  (symbol (lib_id \"{}\") (at {x} {y} {rotation}) (unit 1) (exclude_from_sim no) (in_bom yes) (on_board yes) (dnp no) (uuid \"{}\")", esc(&link.symbol), uuid(index + 1, 0)).unwrap();
        instance_property(&mut out, "Reference", reference, x, y - 5.0, false);
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
            let offset = rotate_pin(*pin, rotation);
            net_points.entry(root).or_default().push(NetPoint {
                point: Point {
                    x: snap(x + offset.x),
                    y: snap(y + offset.y),
                },
                offset,
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
        writeln!(out, "    (instances (project \"etch\" (path \"/{root_uuid}\" (reference \"{}\") (unit 1))))\n  )", esc(reference)).unwrap();
    }
    // A schematic net is a logical hyperedge, not an ordered list of pins.
    // The old exporter converted it into a declaration-order daisy chain,
    // making large shared nets sweep across the whole sheet.  Place a net
    // alias at every terminal instead.  This is the same routing-reduction
    // strategy used by tsCircuit's autolayout and is idiomatic editable KiCad.
    let mut label_id = 1usize;
    for (root, points) in net_points.iter().filter(|(_, points)| points.len() > 1) {
        let mut seen = Vec::<Point>::new();
        for net_point in points {
            if seen.iter().any(|point| same_point(*point, net_point.point)) {
                continue;
            }
            seen.push(net_point.point);
            sch_label(&mut out, names.get(root).unwrap(), *net_point, label_id);
            label_id += 1;
        }
    }
    out.push_str("  (sheet_instances (path \"/\" (page \"1\")))\n  (embedded_fonts no)\n)\n");
    Ok(out)
}

fn place_symbols(
    circuit: &CircuitDesign,
    symbols: &BTreeMap<String, LibrarySymbol>,
    topology: &schematic::SchematicLayout,
) -> Vec<SymbolPosition> {
    const PAGE_TOP: f64 = 20.32;
    const PAGE_BOTTOM: f64 = 573.0;
    const PAGE_LEFT: f64 = 20.32;
    const COLUMN_PITCH: f64 = 190.5;
    const SIDE_GAP: f64 = 20.32;
    const ROW_GAP: f64 = 8.89;
    const SECTION_GAP: f64 = 15.24;

    let roots = circuit.electrical_roots();
    let mut result = vec![
        SymbolPosition {
            x: 0.0,
            y: 0.0,
            rotation: 0
        };
        circuit.components.len()
    ];
    let sections = circuit.section_names();
    let mut column = 0usize;
    let mut cursor_y = PAGE_TOP;

    for section in sections {
        let indices = circuit
            .components
            .iter()
            .enumerate()
            .filter(|(_, component)| component.section.as_deref().unwrap_or("Circuit") == section)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if indices.is_empty() {
            continue;
        }
        let hub = *indices
            .iter()
            .max_by_key(|index| circuit.components[**index].ports.len())
            .unwrap();
        let hub_roots = circuit.components[hub]
            .ports
            .iter()
            .map(|(name, node)| {
                let pin_number = &circuit.components[hub].kicad.as_ref().unwrap().pins[name];
                let pin = symbols[&circuit.components[hub].kicad.as_ref().unwrap().symbol].pins
                    [pin_number];
                (roots.get(node).copied().unwrap_or(*node), pin)
            })
            .collect::<Vec<_>>();

        let mut left = Vec::new();
        let mut right = Vec::new();
        for index in indices.iter().copied().filter(|index| *index != hub) {
            let component_roots = circuit.components[index]
                .ports
                .values()
                .map(|node| roots.get(node).copied().unwrap_or(*node))
                .collect::<Vec<_>>();
            let connected_pins = hub_roots
                .iter()
                .filter(|(root, _)| component_roots.contains(root))
                .map(|(_, pin)| *pin)
                .collect::<Vec<_>>();
            let order = connected_pins
                .iter()
                .map(|pin| pin.y)
                .min_by(f64::total_cmp)
                .unwrap_or(f64::MAX);
            let side_score = connected_pins
                .iter()
                .map(|pin| pin.x.signum() as i32)
                .sum::<i32>();
            if side_score < 0 || (side_score == 0 && left.len() <= right.len()) {
                left.push((index, order));
            } else {
                right.push((index, order));
            }
        }
        left.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        right.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));

        let size = |index: usize| {
            symbol_size(
                &symbols[&circuit.components[index].kicad.as_ref().unwrap().symbol],
                topology.components[index].rotation,
            )
        };
        let (hub_width, hub_height) = size(hub);
        let left_width = left
            .iter()
            .map(|(index, _)| size(*index).0)
            .reduce(f64::max)
            .unwrap_or(0.0);
        let right_width = right
            .iter()
            .map(|(index, _)| size(*index).0)
            .reduce(f64::max)
            .unwrap_or(0.0);
        let stack_height = |items: &[(usize, f64)]| {
            items.iter().map(|(index, _)| size(*index).1).sum::<f64>()
                + items.len().saturating_sub(1) as f64 * ROW_GAP
        };
        let section_height = hub_height
            .max(stack_height(&left))
            .max(stack_height(&right));
        if cursor_y + section_height > PAGE_BOTTOM && cursor_y > PAGE_TOP {
            column += 1;
            cursor_y = PAGE_TOP;
        }
        let origin_x = PAGE_LEFT + column as f64 * COLUMN_PITCH;
        let hub_x = origin_x + left_width + SIDE_GAP + hub_width / 2.0;
        let section_center_y = cursor_y + section_height / 2.0;
        result[hub] = SymbolPosition {
            x: snap(hub_x),
            y: snap(section_center_y),
            rotation: topology.components[hub].rotation,
        };
        let mut place_stack = |items: &[(usize, f64)], x: f64| {
            let mut y = cursor_y;
            for (index, _) in items {
                let (_, height) = size(*index);
                result[*index] = SymbolPosition {
                    x: snap(x),
                    y: snap(y + height / 2.0),
                    rotation: topology.components[*index].rotation,
                };
                y += height + ROW_GAP;
            }
        };
        place_stack(&left, origin_x + left_width / 2.0);
        place_stack(
            &right,
            hub_x + hub_width / 2.0 + SIDE_GAP + right_width / 2.0,
        );
        cursor_y += section_height + SECTION_GAP;
    }
    result
}

fn symbol_size(symbol: &LibrarySymbol, rotation: i32) -> (f64, f64) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for pin in symbol.pins.values() {
        let point = rotate_pin(*pin, rotation);
        xs.push(point.x);
        ys.push(point.y);
    }
    let width = xs.iter().copied().reduce(f64::max).unwrap_or(0.0)
        - xs.iter().copied().reduce(f64::min).unwrap_or(0.0)
        + 12.7;
    let height = ys.iter().copied().reduce(f64::max).unwrap_or(0.0)
        - ys.iter().copied().reduce(f64::min).unwrap_or(0.0)
        + 12.7;
    (width.max(12.7), height.max(12.7))
}

#[derive(Clone, Copy)]
struct NetPoint {
    point: Point,
    offset: Point,
}

fn snap(value: f64) -> f64 {
    const GRID: f64 = 1.27;
    (value / GRID).round() * GRID
}

fn rotate_pin(pin: Pin, rotation: i32) -> Point {
    // Library coordinates point up; schematic page coordinates point down.
    match rotation.rem_euclid(360) {
        90 => Point {
            x: -pin.y,
            y: -pin.x,
        },
        180 => Point {
            x: -pin.x,
            y: pin.y,
        },
        270 => Point { x: pin.y, y: pin.x },
        _ => Point {
            x: pin.x,
            y: -pin.y,
        },
    }
}

fn sch_label(out: &mut String, name: &str, net_point: NetPoint, id: usize) {
    let horizontal = net_point.offset.x.abs() >= net_point.offset.y.abs();
    let (rotation, justify) = if horizontal {
        if net_point.offset.x < 0.0 {
            (0, "right bottom")
        } else {
            (0, "left bottom")
        }
    } else if net_point.offset.y < 0.0 {
        (90, "right bottom")
    } else {
        (90, "left bottom")
    };
    writeln!(
        out,
        "  (label \"{}\" (at {} {} {rotation}) (effects (font (size 1.27 1.27)) (justify {justify})) (uuid \"{}\"))",
        esc(name),
        net_point.point.x,
        net_point.point.y,
        uuid(9000, id)
    )
    .unwrap();
}

fn same_point(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001
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
