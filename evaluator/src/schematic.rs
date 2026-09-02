use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

use super::{CircuitState, SchematicComponent};

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

struct Placed<'a> {
    component: &'a SchematicComponent,
    center: Point,
    section: String,
    ports: BTreeMap<u64, Point>,
}

pub(super) fn render(circuit: &CircuitState) -> String {
    let sections = section_names(circuit);
    let roots = electrical_roots(circuit);
    let mut placed = Vec::new();
    let mut y_offset = 70.0;
    let mut max_width: f64 = 520.0;

    for section in &sections {
        let components = circuit
            .schematic_components
            .iter()
            .filter(|component| component.section.as_deref().unwrap_or("Circuit") == section)
            .collect::<Vec<_>>();
        let layers = graph_layers(&components, &roots);
        let max_layer = layers.iter().copied().max().unwrap_or(0);
        let mut rows = BTreeMap::<usize, usize>::new();
        for (component, layer) in components.into_iter().zip(layers) {
            let row = rows.entry(layer).or_default();
            let center = Point {
                x: 110.0 + layer as f64 * 190.0,
                y: y_offset + 70.0 + *row as f64 * 115.0,
            };
            *row += 1;
            placed.push(Placed {
                component,
                center,
                section: section.clone(),
                ports: port_points(component, center),
            });
        }
        let height = rows.values().copied().max().unwrap_or(1) as f64 * 115.0 + 65.0;
        y_offset += height + 45.0;
        max_width = max_width.max(220.0 + max_layer as f64 * 190.0);
    }

    let width = max_width + 80.0;
    let height = y_offset.max(220.0);
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    )
    .unwrap();
    svg.push_str(STYLE_AND_SYMBOLS);
    write_custom_symbols(&mut svg, &placed);
    draw_sections(&mut svg, &sections, &placed, width);
    draw_wires(&mut svg, circuit, &placed, &roots);
    draw_components(&mut svg, &placed);
    svg.push_str("</svg>\n");
    svg
}

fn section_names(circuit: &CircuitState) -> Vec<String> {
    let mut sections = circuit.sections.clone();
    if circuit
        .schematic_components
        .iter()
        .any(|component| component.section.is_none())
    {
        sections.insert(0, "Circuit".into());
    }
    if sections.is_empty() {
        sections.push("Circuit".into());
    }
    sections
}

fn electrical_roots(circuit: &CircuitState) -> BTreeMap<u64, u64> {
    let mut roots = circuit
        .voltages
        .keys()
        .map(|id| (*id, *id))
        .collect::<BTreeMap<_, _>>();
    for (a, connected) in &circuit.connections {
        for b in connected {
            union(&mut roots, *a, *b);
        }
    }
    let ids = roots.keys().copied().collect::<Vec<_>>();
    for id in ids {
        let root = find(&roots, id);
        roots.insert(id, root);
    }
    roots
}

fn find(roots: &BTreeMap<u64, u64>, mut id: u64) -> u64 {
    while roots.get(&id).copied().unwrap_or(id) != id {
        id = roots[&id];
    }
    id
}

fn union(roots: &mut BTreeMap<u64, u64>, a: u64, b: u64) {
    let a = find(roots, a);
    let b = find(roots, b);
    if a != b {
        let (low, high) = if a < b { (a, b) } else { (b, a) };
        roots.insert(high, low);
    }
}

fn graph_layers(components: &[&SchematicComponent], roots: &BTreeMap<u64, u64>) -> Vec<usize> {
    if components.is_empty() {
        return Vec::new();
    }
    let nets = components
        .iter()
        .map(|component| {
            component
                .ports
                .values()
                .map(|node| roots.get(node).copied().unwrap_or(*node))
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    let mut adjacency = vec![Vec::new(); components.len()];
    for a in 0..components.len() {
        for b in (a + 1)..components.len() {
            if !nets[a].is_disjoint(&nets[b]) {
                adjacency[a].push(b);
                adjacency[b].push(a);
            }
        }
    }
    let starts = components
        .iter()
        .enumerate()
        .filter(|(_, component)| {
            component.kind.contains("source")
                || component.kind.contains("input")
                || component.kind == "ground"
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut layers = vec![usize::MAX; components.len()];
    let mut queue = VecDeque::new();
    for start in starts.into_iter().chain(std::iter::once(0)) {
        if layers[start] == usize::MAX {
            layers[start] = 0;
            queue.push_back(start);
        }
    }
    while let Some(component) = queue.pop_front() {
        for next in &adjacency[component] {
            if layers[*next] == usize::MAX {
                layers[*next] = layers[component] + 1;
                queue.push_back(*next);
            }
        }
    }
    let mut fallback = layers
        .iter()
        .filter(|layer| **layer != usize::MAX)
        .copied()
        .max()
        .unwrap_or(0);
    for layer in &mut layers {
        if *layer == usize::MAX {
            fallback += 1;
            *layer = fallback;
        }
    }
    layers
}

fn port_points(component: &SchematicComponent, center: Point) -> BTreeMap<u64, Point> {
    let count = component.ports.len();
    component
        .ports
        .iter()
        .enumerate()
        .map(|(index, (name, node))| {
            let point = port_point(component, name, index, count, center);
            (*node, point)
        })
        .collect()
}

fn port_point(
    component: &SchematicComponent,
    name: &str,
    index: usize,
    count: usize,
    center: Point,
) -> Point {
    let (x, y) = match (component.kind.as_str(), name) {
        ("ground", _) => (0.0, 30.0),
        ("op-amp", "positive") => (-40.0, -14.0),
        ("op-amp", "negative") => (-40.0, 14.0),
        ("op-amp" | "comparator", "input") => (-40.0, 0.0),
        ("op-amp" | "comparator", "output") => (40.0, 0.0),
        ("and-gate" | "or-gate" | "nand-gate" | "nor-gate" | "d-flip-flop", "a" | "d") => {
            (-40.0, -12.0)
        }
        ("and-gate" | "or-gate" | "nand-gate" | "nor-gate" | "d-flip-flop", "b" | "clock") => {
            (-40.0, 12.0)
        }
        ("not-gate", "input") => (-40.0, 0.0),
        (
            "not-gate" | "and-gate" | "or-gate" | "nand-gate" | "nor-gate" | "d-flip-flop",
            "output" | "q",
        ) => (40.0, 0.0),
        (_, "b" | "cathode" | "negative") => (40.0, 0.0),
        (_, "a" | "anode" | "positive") => (-40.0, 0.0),
        _ => (
            if index + 1 == count { 40.0 } else { -40.0 },
            (index as f64 - (count.saturating_sub(1)) as f64 / 2.0) * 16.0,
        ),
    };
    Point {
        x: center.x + x,
        y: center.y + y,
    }
}

fn draw_sections(svg: &mut String, sections: &[String], placed: &[Placed<'_>], width: f64) {
    for section in sections {
        let points = placed
            .iter()
            .filter(|item| &item.section == section)
            .map(|item| item.center.y)
            .collect::<Vec<_>>();
        let min = points.iter().copied().reduce(f64::min).unwrap_or(100.0) - 55.0;
        let max = points.iter().copied().reduce(f64::max).unwrap_or(100.0) + 55.0;
        writeln!(
            svg,
            r#"<g class="section"><rect x="20" y="{min}" width="{}" height="{}" rx="8"/><text x="34" y="{}">{}</text></g>"#,
            width - 40.0,
            max - min,
            min + 20.0,
            escape(section)
        )
        .unwrap();
    }
}

fn draw_wires(
    svg: &mut String,
    circuit: &CircuitState,
    placed: &[Placed<'_>],
    roots: &BTreeMap<u64, u64>,
) {
    let mut endpoints = BTreeMap::<u64, Vec<(&str, Point)>>::new();
    for item in placed {
        for (node, point) in &item.ports {
            endpoints
                .entry(roots.get(node).copied().unwrap_or(*node))
                .or_default()
                .push((&item.section, *point));
        }
    }
    let labels = circuit
        .net_labels
        .iter()
        .flat_map(|(label, nodes)| {
            nodes
                .iter()
                .map(|node| (roots.get(node).copied().unwrap_or(*node), label.as_str()))
        })
        .collect::<BTreeMap<_, _>>();

    for (root, points) in endpoints {
        let section_count = points
            .iter()
            .map(|(section, _)| *section)
            .collect::<BTreeSet<_>>()
            .len();
        if let Some(label) = labels.get(&root) {
            for (_, point) in &points {
                writeln!(
                    svg,
                    r#"<g class="net-label"><path d="M {} {} h 18"/><text x="{}" y="{}">{}</text></g>"#,
                    point.x,
                    point.y,
                    point.x + 21.0,
                    point.y - 4.0,
                    escape(label)
                )
                .unwrap();
            }
        }
        if points.len() > 1 && section_count == 1 {
            let trunk_x =
                points.iter().map(|(_, point)| point.x).sum::<f64>() / points.len() as f64;
            let min_y = points
                .iter()
                .map(|(_, point)| point.y)
                .reduce(f64::min)
                .unwrap();
            let max_y = points
                .iter()
                .map(|(_, point)| point.y)
                .reduce(f64::max)
                .unwrap();
            writeln!(
                svg,
                r#"<path class="wire" d="M {trunk_x} {min_y} V {max_y}"/>"#
            )
            .unwrap();
            for (_, point) in points {
                writeln!(
                    svg,
                    r#"<path class="wire" d="M {} {} H {trunk_x}"/><circle class="junction" cx="{trunk_x}" cy="{}" r="2.5"/>"#,
                    point.x, point.y, point.y
                )
                .unwrap();
            }
        }
    }
}

fn draw_components(svg: &mut String, placed: &[Placed<'_>]) {
    let mut counts = BTreeMap::<&str, usize>::new();
    for (index, item) in placed.iter().enumerate() {
        let count = counts.entry(&item.component.kind).or_default();
        *count += 1;
        let label = item
            .component
            .label
            .clone()
            .unwrap_or_else(|| format!("{}{}", prefix(&item.component.kind), count));
        let symbol = if item.component.svg.is_some() {
            format!("custom-{index}")
        } else {
            symbol_id(&item.component.kind)
        };
        writeln!(
            svg,
            r##"<g class="component"><use href="#{symbol}" x="{}" y="{}" width="80" height="60"/><text class="reference" x="{}" y="{}">{}</text>"##,
            item.center.x - 40.0,
            item.center.y - 30.0,
            item.center.x,
            item.center.y - 38.0,
            escape(&label)
        )
        .unwrap();
        if let Some(value) = &item.component.value {
            writeln!(
                svg,
                r#"<text class="value" x="{}" y="{}">{}</text>"#,
                item.center.x,
                item.center.y + 44.0,
                escape(value)
            )
            .unwrap();
        }
        svg.push_str("</g>\n");
    }
}

fn write_custom_symbols(svg: &mut String, placed: &[Placed<'_>]) {
    svg.push_str("<defs>\n");
    for (index, item) in placed.iter().enumerate() {
        if let Some(fragment) = &item.component.svg {
            writeln!(
                svg,
                r#"<symbol id="custom-{index}" viewBox="-40 -30 80 60">{fragment}</symbol>"#
            )
            .unwrap();
        }
    }
    svg.push_str("</defs>\n");
}

fn symbol_id(kind: &str) -> String {
    match kind {
        "voltage-source" | "current-source" | "ground" | "resistor" | "capacitor" | "inductor"
        | "switch" | "op-amp" | "comparator" | "diode" | "led" | "not-gate" | "and-gate"
        | "or-gate" | "nand-gate" | "nor-gate" | "d-flip-flop" => format!("symbol-{kind}"),
        _ => "symbol-generic".into(),
    }
}

fn prefix(kind: &str) -> &'static str {
    match kind {
        "resistor" => "R",
        "capacitor" => "C",
        "inductor" => "L",
        "diode" | "led" => "D",
        "voltage-source" | "current-source" => "V",
        "ground" => "GND",
        "switch" => "SW",
        "op-amp" | "comparator" => "U",
        _ => "U",
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE_AND_SYMBOLS: &str = r##"
<style>
  .section rect { fill:#fbfcfe; stroke:#aab3c2; stroke-dasharray:6 4; }
  .section text,.reference,.value,.net-label text { font-family:ui-monospace,monospace; fill:#172033; }
  .section text { font-size:14px; font-weight:600; }
  .reference { font-size:12px; font-weight:600; text-anchor:middle; }
  .value { font-size:11px; text-anchor:middle; }
  .wire,.net-label path { fill:none; stroke:#245c3b; stroke-width:2; }
  .junction { fill:#245c3b; }
  .net-label text { font-size:11px; }
  symbol * { vector-effect:non-scaling-stroke; }
</style>
<defs>
  <symbol id="symbol-generic" viewBox="-40 -30 80 60"><path d="M-40 0h15M25 0h15M-25-20h50v40h-50z" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-resistor" viewBox="-40 -30 80 60"><path d="M-40 0h10l5-12 10 24 10-24 10 24 10-24 10 24 5-12h10" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-capacitor" viewBox="-40 -30 80 60"><path d="M-40 0h30M-10-20v40M10-20v40M10 0h30" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-inductor" viewBox="-40 -30 80 60"><path d="M-40 0h8c0-20 16-20 16 0 0-20 16-20 16 0 0-20 16-20 16 0 0-20 16-20 16 0h8" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-diode" viewBox="-40 -30 80 60"><path d="M-40 0h22M18 0h22M-18-18v36L18 0zM18-18v36" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-led" viewBox="-40 -30 80 60"><path d="M-40 0h22M18 0h22M-18-18v36L18 0zM18-18v36M5-20l12-10m-4 2 4-2-1 5M15-12l12-10m-4 2 4-2-1 5" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-voltage-source" viewBox="-40 -30 80 60"><path d="M-40 0h10M30 0h10M-30 0a30 30 0 1 0 60 0 30 30 0 1 0-60 0M-12-8h12M-6-14v12M8 8h12" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-current-source" viewBox="-40 -30 80 60"><path d="M-40 0h10M30 0h10M-30 0a30 30 0 1 0 60 0 30 30 0 1 0-60 0M-12 0h24m-8-7 8 7-8 7" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-ground" viewBox="-40 -30 80 60"><path d="M0-25V5M-22 5h44M-14 13h28M-6 21h12" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-switch" viewBox="-40 -30 80 60"><path d="M-40 0h15M25 0h15M-25 0L18-18M-25 0h2M23 0h2" fill="none" stroke="#172033" stroke-width="2"/><circle cx="-25" cy="0" r="3"/><circle cx="25" cy="0" r="3"/></symbol>
  <symbol id="symbol-op-amp" viewBox="-40 -30 80 60"><path d="M-40-14h14M-40 14h14M28 0h12M-26-27v54L28 0zM-20-14h8M-16-18v8M-20 14h8" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-comparator" viewBox="-40 -30 80 60"><path d="M-40-14h14M-40 14h14M28 0h12M-26-27v54L28 0zM-20-14h8M-20 14h8" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-not-gate" viewBox="-40 -30 80 60"><path d="M-40 0h14M-26-22v44L20 0zM28 0h12" fill="white" stroke="#172033" stroke-width="2"/><circle cx="24" cy="0" r="4" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-and-gate" viewBox="-40 -30 80 60"><path d="M-40-12h14M-40 12h14M26 0h14M-26-24v48h22a24 24 0 0 0 0-48z" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-or-gate" viewBox="-40 -30 80 60"><path d="M-40-12h18M-40 12h18M25 0h15M-26-24q12 24 0 48 32 0 51-24-19-24-51-24z" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-nand-gate" viewBox="-40 -30 80 60"><path d="M-40-12h14M-40 12h14M-26-24v48h20a24 24 0 0 0 0-48zM30 0h10" fill="white" stroke="#172033" stroke-width="2"/><circle cx="26" cy="0" r="4" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-nor-gate" viewBox="-40 -30 80 60"><path d="M-40-12h18M-40 12h18M-26-24q12 24 0 48 30 0 47-24-17-24-47-24zM29 0h11" fill="white" stroke="#172033" stroke-width="2"/><circle cx="25" cy="0" r="4" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-d-flip-flop" viewBox="-40 -30 80 60"><path d="M-40-12h15M-40 12h15M25-12h15M-25-25h50v50h-50zM-25 6l8 6-8 6" fill="white" stroke="#172033" stroke-width="2"/><text x="-18" y="-8" font-size="10">D</text><text x="13" y="-8" font-size="10">Q</text></symbol>
</defs>
"##;
