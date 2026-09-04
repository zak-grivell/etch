pub(super) fn draw_sections(
    svg: &mut String,
    sections: &[String],
    placed: &[Placed<'_>],
    width: f64,
) {
    for section in sections {
        let items = placed
            .iter()
            .filter(|item| &item.section == section)
            .collect::<Vec<_>>();
        let min = items
            .iter()
            .map(|item| component_hitbox(item, 0.0).top)
            .reduce(f64::min)
            .unwrap_or(100.0)
            - 42.0;
        let max = items
            .iter()
            .map(|item| component_hitbox(item, 0.0).bottom)
            .reduce(f64::max)
            .unwrap_or(100.0)
            + 42.0;
        writeln!(
            svg,
            r#"<g class="section"><rect x="20" y="{min}" width="{}" height="{}" rx="8"/><text x="34" y="{}">{}</text></g>"#,
            width - 40.0,
            max - min,
            min + 20.0,
            escape_xml(section)
        )
        .unwrap();
    }
}

pub(super) fn draw_wires(
    svg: &mut String,
    circuit: &CircuitDesign,
    placed: &[Placed<'_>],
    roots: &BTreeMap<u64, u64>,
) {
    let mut endpoints = BTreeMap::<u64, Vec<(&str, Point, bool)>>::new();
    for item in placed {
        for (node, point) in &item.ports {
            endpoints
                .entry(roots.get(node).copied().unwrap_or(*node))
                .or_default()
                .push((
                    &item.section,
                    *point,
                    matches!(
                        item.orientation,
                        Orientation::Ground
                            | Orientation::Supply
                            | Orientation::AnchorSupply
                            | Orientation::AnchorGround
                    ),
                ));
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
        let is_ground_net = placed.iter().any(|item| {
            matches!(
                item.orientation,
                Orientation::Ground | Orientation::AnchorGround
            ) && item
                .component
                .ports
                .values()
                .any(|node| roots.get(node).copied().unwrap_or(*node) == root)
        });
        let is_power_net = !is_ground_net
            && labels.contains_key(&root)
            && placed.iter().any(|item| {
                matches!(
                    item.orientation,
                    Orientation::Supply | Orientation::AnchorSupply
                ) && item
                    .component
                    .ports
                    .values()
                    .any(|node| roots.get(node).copied().unwrap_or(*node) == root)
            });
        let section_count = points
            .iter()
            .map(|(section, _, _)| *section)
            .collect::<BTreeSet<_>>()
            .len();
        if is_ground_net || is_power_net {
            let power_value = placed.iter().find_map(|item| {
                (matches!(
                    item.orientation,
                    Orientation::Supply | Orientation::AnchorSupply
                ) && item
                    .component
                    .ports
                    .values()
                    .any(|node| roots.get(node).copied().unwrap_or(*node) == root))
                .then_some(item.component.value.as_deref())
                .flatten()
            });
            let mut terminal_sections = BTreeSet::new();
            for (section, point, is_terminal) in &points {
                let mut marker_point = *point;
                if *is_terminal {
                    let has_local_consumer =
                        points.iter().any(|(other_section, _, other_is_terminal)| {
                            other_section == section && !*other_is_terminal
                        });
                    if has_local_consumer || !terminal_sections.insert(*section) {
                        continue;
                    }
                    if is_ground_net {
                        marker_point.x += 45.0;
                        marker_point.y += 15.0;
                    } else {
                        marker_point.x -= 30.0;
                        marker_point.y += 25.0;
                    }
                }
                if is_ground_net {
                    draw_ground_marker(svg, marker_point);
                } else {
                    draw_power_marker(svg, marker_point, power_value);
                }
            }
            continue;
        }
        if section_count > 1
            && let Some(label) = labels.get(&root)
        {
            for (_, point, is_terminal) in &points {
                if *is_terminal {
                    continue;
                }
                let side = placed
                    .iter()
                    .find(|item| item.ports.values().any(|port| same_point(*port, *point)))
                    .map(|item| if point.x < item.center.x { -1 } else { 1 })
                    .unwrap_or(1);
                draw_net_tag(svg, *point, side, label);
            }
            continue;
        }
        if points.len() > 1 && section_count == 1 {
            let route_points = points
                .iter()
                .map(|(_, point, _)| *point)
                .collect::<Vec<_>>();
            draw_orthogonal_net(svg, &route_points, placed, circuit, roots);
        }
    }
}

pub(super) fn draw_power_marker(svg: &mut String, point: Point, value: Option<&str>) {
    let rail_y = point.y - 40.0;
    writeln!(
        svg,
        r#"<g class="power-marker"><path d="M {} {} V {rail_y} M {} {rail_y} H {}"/>"#,
        point.x,
        point.y,
        point.x - 18.0,
        point.x + 18.0
    )
    .unwrap();
    if let Some(value) = value {
        writeln!(
            svg,
            r#"<text class="supply-value" x="{}" y="{}">{}</text>"#,
            point.x,
            rail_y - 7.0,
            escape_xml(&component_value(value))
        )
        .unwrap();
    }
    svg.push_str("</g>\n");
}

pub(super) fn draw_ground_marker(svg: &mut String, point: Point) {
    writeln!(
        svg,
        r##"<use class="ground-marker" href="#symbol-ground" xlink:href="#symbol-ground" x="{}" y="{}" width="80" height="60"/>"##,
        point.x - 40.0,
        point.y - 5.0
    )
    .unwrap();
}

pub(super) fn draw_net_tag(svg: &mut String, point: Point, side: i8, label: &str) {
    let width = label.chars().count() as f64 * 7.2 + 14.0;
    let direction = side as f64;
    let tip_x = point.x + direction * 8.0;
    let far_x = tip_x + direction * width;
    let text_x = tip_x + direction * 7.0;
    let anchor = if side < 0 { "end" } else { "start" };
    writeln!(
        svg,
        r#"<g class="net-label"><path d="M {} {} L {tip_x} {} H {far_x} V {} H {tip_x} Z"/><text text-anchor="{anchor}" x="{text_x}" y="{}">{}</text></g>"#,
        point.x,
        point.y,
        point.y - 9.0,
        point.y + 9.0,
        point.y + 4.0,
        escape_xml(label)
    )
    .unwrap();
}

pub(super) fn draw_components(
    svg: &mut String,
    circuit: &CircuitDesign,
    placed: &[Placed<'_>],
    roots: &BTreeMap<u64, u64>,
) {
    let mut counts = BTreeMap::<&str, usize>::new();
    for (index, item) in placed.iter().enumerate() {
        if !component_is_visible(circuit, item, roots) {
            continue;
        }
        let count = counts.entry(&item.component.kind).or_default();
        *count += 1;
        let label = item
            .component
            .label
            .clone()
            .unwrap_or_else(|| format!("{}{}", prefix(&item.component.kind), count));
        let clock_above_data = item.component.kind == "d-flip-flop"
            && item
                .component
                .ports
                .get("clock")
                .and_then(|clock| item.ports.get(clock))
                .zip(
                    item.component
                        .ports
                        .get("d")
                        .and_then(|data| item.ports.get(data)),
                )
                .is_some_and(|(clock, data)| clock.y < data.y);
        let symbol = if item.orientation == Orientation::AnchorGround {
            "symbol-ground".into()
        } else if clock_above_data {
            "symbol-d-flip-flop-clock-top".into()
        } else if item.component.svg.is_some() {
            format!("custom-{index}")
        } else {
            symbol_id(&item.component.kind)
        };
        let transform = if item.orientation == Orientation::Vertical {
            format!(
                r#" transform="rotate(90 {} {})""#,
                item.center.x, item.center.y
            )
        } else {
            String::new()
        };
        write!(svg, r#"<g class="component">"#).unwrap();
        if is_generic_component(item.component) {
            draw_generic_component(svg, item);
        } else {
            write!(
                svg,
                r##"<use href="#{symbol}" xlink:href="#{symbol}" x="{}" y="{}" width="{SYMBOL_WIDTH}" height="{SYMBOL_HEIGHT}"{transform}/>"##,
                item.center.x - SYMBOL_WIDTH / 2.0,
                item.center.y - SYMBOL_HEIGHT / 2.0,
            )
            .unwrap();
        }
        let show_reference = item.component.kind != "voltage-source"
            && (item.component.kind != "ground" || item.component.label.is_some());
        if show_reference {
            let (_, item_height) = component_size(item.component);
            let (reference_x, reference_y, anchor) = match item.orientation {
                Orientation::Vertical => (item.center.x - 38.0, item.center.y - 7.0, "end"),
                Orientation::Ground => (item.center.x + 34.0, item.center.y + 4.0, "start"),
                _ => (
                    item.center.x,
                    item.center.y - item_height / 2.0 - 8.0,
                    "middle",
                ),
            };
            writeln!(
                svg,
                r#"<text class="reference" text-anchor="{anchor}" x="{reference_x}" y="{reference_y}">{}</text>"#,
                escape_xml(&label)
            )
            .unwrap();
        }
        if item.orientation != Orientation::AnchorGround
            && let Some(value) = &item.component.value
        {
            let value = component_value(value);
            if item.component.kind == "voltage-source" {
                writeln!(
                    svg,
                    r#"<text class="supply-value" x="{}" y="{}">{}</text>"#,
                    item.center.x,
                    item.center.y - 12.0,
                    escape_xml(&value)
                )
                .unwrap();
            } else {
                let (_, item_height) = component_size(item.component);
                let (value_x, value_y, anchor) = if item.orientation == Orientation::Vertical {
                    (item.center.x + 38.0, item.center.y + 4.0, "start")
                } else {
                    (
                        item.center.x,
                        item.center.y + item_height / 2.0 + 14.0,
                        "middle",
                    )
                };
                writeln!(
                    svg,
                    r#"<text class="value" text-anchor="{anchor}" x="{value_x}" y="{value_y}">{}</text>"#,
                    escape_xml(&value)
                )
                .unwrap();
            }
        }
        svg.push_str("</g>\n");
    }
}

fn draw_generic_component(svg: &mut String, item: &Placed<'_>) {
    let (width, height) = component_size(item.component);
    writeln!(
        svg,
        r#"<rect class="ic-body" x="{}" y="{}" width="{width}" height="{height}" rx="4"/>"#,
        item.center.x - width / 2.0,
        item.center.y - height / 2.0,
    )
    .unwrap();
    let count = item.component.ports.len();
    for (index, (name, _)) in item.component.ports.iter().enumerate() {
        let (x, y) = generic_port_offset(item.component, index, count);
        let inward = if x < 0.0 { 12.0 } else { -12.0 };
        let anchor = if x < 0.0 { "start" } else { "end" };
        writeln!(
            svg,
            r#"<path class="pin" d="M {} {} h {inward}"/><text class="pin-name" text-anchor="{anchor}" x="{}" y="{}">{}</text>"#,
            item.center.x + x,
            item.center.y + y,
            item.center.x + x + inward + if x < 0.0 { 3.0 } else { -3.0 },
            item.center.y + y + 3.5,
            escape_xml(name),
        )
        .unwrap();
    }
}

pub(super) fn component_is_visible(
    circuit: &CircuitDesign,
    item: &Placed<'_>,
    roots: &BTreeMap<u64, u64>,
) -> bool {
    if item.orientation == Orientation::Ground {
        return false;
    }
    if !matches!(
        item.orientation,
        Orientation::Supply | Orientation::AnchorSupply
    ) {
        return true;
    }
    let Some(positive) = item.component.ports.get("positive") else {
        return true;
    };
    let positive_root = roots.get(positive).copied().unwrap_or(*positive);
    !circuit
        .net_labels
        .values()
        .flatten()
        .any(|node| roots.get(node).copied().unwrap_or(*node) == positive_root)
}

pub(super) fn write_custom_symbols(svg: &mut String, placed: &[Placed<'_>]) {
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

pub(super) fn symbol_id(kind: &str) -> String {
    match kind {
        "voltage-source" | "current-source" | "ground" | "resistor" | "capacitor" | "inductor"
        | "switch" | "op-amp" | "comparator" | "diode" | "led" | "not-gate" | "and-gate"
        | "or-gate" | "nand-gate" | "nor-gate" | "d-flip-flop" => format!("symbol-{kind}"),
        _ => "symbol-generic".into(),
    }
}

pub(super) fn prefix(kind: &str) -> &'static str {
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

pub(super) fn component_value(value: &str) -> String {
    let (number, unit) = match value.split_once(' ') {
        Some((number, unit)) => (number, unit),
        None => (value, ""),
    };
    let Ok(number) = number.parse::<f64>() else {
        return value.to_owned();
    };
    if !number.is_finite() {
        return value.to_owned();
    }

    let (scaled, engineering_prefix) = engineering(number);
    format!("{} {engineering_prefix}{unit}", concise_number(scaled))
        .trim_end()
        .to_owned()
}

pub(super) fn engineering(value: f64) -> (f64, &'static str) {
    if value == 0.0 {
        return (0.0, "");
    }
    const PREFIXES: [(i32, &str); 9] = [
        (-12, "p"),
        (-9, "n"),
        (-6, "µ"),
        (-3, "m"),
        (0, ""),
        (3, "k"),
        (6, "M"),
        (9, "G"),
        (12, "T"),
    ];
    let exponent = ((value.abs().log10() / 3.0).floor() as i32 * 3).clamp(-12, 12);
    let prefix = PREFIXES
        .iter()
        .find(|(candidate, _)| *candidate == exponent)
        .map(|(_, prefix)| *prefix)
        .unwrap_or("");
    (value / 10_f64.powi(exponent), prefix)
}

pub(super) fn concise_number(value: f64) -> String {
    let rendered = format!("{value:.3}");
    rendered
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

pub(super) const STYLE_AND_SYMBOLS: &str = r##"
<style>
  .section rect { fill:#fbfcfe; stroke:#aab3c2; stroke-dasharray:6 4; }
  .section text,.reference,.value,.supply-value,.net-label text { font-family:ui-monospace,monospace; fill:#172033; }
  .section text { font-size:14px; font-weight:600; }
  .reference { font-size:12px; font-weight:600; }
  .value { font-size:11px; }
  .supply-value { font-size:12px; font-weight:600; text-anchor:middle; paint-order:stroke; stroke:#fbfcfe; stroke-width:6px; stroke-linejoin:round; }
  .wire,.net-label path { fill:none; stroke:#172033; stroke-width:2; }
  .net-label path { fill:#fbfcfe; }
  .junction { fill:#172033; }
  .net-label text { font-size:11px; }
  .power-marker path { fill:none; stroke:#172033; stroke-width:2; }
  .ic-body { fill:white; stroke:#172033; stroke-width:2; }
  .pin { fill:none; stroke:#172033; stroke-width:2; }
  .pin-name { font-family:ui-monospace,monospace; font-size:9px; fill:#526077; }
  symbol * { vector-effect:non-scaling-stroke; }
</style>
<defs>
  <symbol id="symbol-generic" data-ports="a:-40,0;b:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h15M25 0h15M-25-20h50v40h-50z" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-resistor" data-ports="a:-40,0;b:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h10l5-12 10 24 10-24 10 24 10-24 10 24 5-12h10" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-capacitor" data-ports="a:-40,0;b:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h30M-10-20v40M10-20v40M10 0h30" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-inductor" data-ports="a:-40,0;b:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h8c0-20 16-20 16 0 0-20 16-20 16 0 0-20 16-20 16 0 0-20 16-20 16 0h8" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-diode" data-ports="anode:-40,0;cathode:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h22M18 0h22M-18-18v36L18 0zM18-18v36" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-led" data-ports="anode:-40,0;cathode:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h22M18 0h22M-18-18v36L18 0zM18-18v36M5-20l12-10m-4 2 4-2-1 5M15-12l12-10m-4 2 4-2-1 5" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-voltage-source" data-ports="positive:0,30;negative:-30,0" viewBox="-40 -30 80 60"><path d="M-30 0H30M0 0V30" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-voltage-source-horizontal" data-ports="positive:30,0;negative:-30,0" viewBox="-40 -30 80 60"><path d="M-30 0H30" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-current-source" data-ports="positive:40,0;negative:-40,0" viewBox="-40 -30 80 60"><path d="M-40 0h10M30 0h10M-30 0a30 30 0 1 0 60 0 30 30 0 1 0-60 0M-12 0h24m-8-7 8 7-8 7" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-ground" data-ports="node:0,-25" viewBox="-40 -30 80 60"><path d="M0-25V5M-22 5h44M-14 13h28M-6 21h12" fill="none" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-switch" data-ports="a:-40,0;b:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h15M25 0h15M-25 0L18-18M-25 0h2M23 0h2" fill="none" stroke="#172033" stroke-width="2"/><circle cx="-25" cy="0" r="3"/><circle cx="25" cy="0" r="3"/></symbol>
  <symbol id="symbol-op-amp" data-ports="positive:-40,-14;negative:-40,14;output:40,0" viewBox="-40 -30 80 60"><path d="M-40-14h14M-40 14h14M28 0h12M-26-27v54L28 0zM-20-14h8M-16-18v8M-20 14h8" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-comparator" data-ports="input:-40,0;output:40,0" viewBox="-40 -30 80 60"><path d="M-40-14h14M-40 14h14M28 0h12M-26-27v54L28 0zM-20-14h8M-20 14h8" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-not-gate" data-ports="input:-40,0;output:40,0" viewBox="-40 -30 80 60"><path d="M-40 0h14M-26-22v44L20 0zM28 0h12" fill="white" stroke="#172033" stroke-width="2"/><circle cx="24" cy="0" r="4" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-and-gate" data-ports="a:-40,-18;b:-40,18;output:40,0" viewBox="-40 -30 80 60"><path d="M-40-18h14M-40 18h14M26 0h14M-26-24v48h22a24 24 0 0 0 0-48z" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-or-gate" data-ports="a:-40,-18;b:-40,18;output:40,0" viewBox="-40 -30 80 60"><path d="M-40-18h18M-40 18h18M25 0h15M-26-24q12 24 0 48 32 0 51-24-19-24-51-24z" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-nand-gate" data-ports="a:-40,-18;b:-40,18;output:40,0" viewBox="-40 -30 80 60"><path d="M-40-18h14M-40 18h14M-26-24v48h20a24 24 0 0 0 0-48zM30 0h10" fill="white" stroke="#172033" stroke-width="2"/><circle cx="26" cy="0" r="4" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-nor-gate" data-ports="a:-40,-18;b:-40,18;output:40,0" viewBox="-40 -30 80 60"><path d="M-40-18h18M-40 18h18M-26-24q12 24 0 48 30 0 47-24-17-24-47-24zM29 0h11" fill="white" stroke="#172033" stroke-width="2"/><circle cx="25" cy="0" r="4" fill="white" stroke="#172033" stroke-width="2"/></symbol>
  <symbol id="symbol-d-flip-flop" data-ports="d:-40,-18;clock:-40,18;q:40,-18" viewBox="-40 -30 80 60"><path d="M-40-18h15M-40 18h15M25-18h15M-25-25h50v50h-50zM-25 12l8 6-8 6" fill="white" stroke="#172033" stroke-width="2"/><text x="-18" y="-12" font-size="10">D</text><text x="13" y="-12" font-size="10">Q</text></symbol>
  <symbol id="symbol-d-flip-flop-clock-top" data-ports="clock:-40,-18;d:-40,18;q:40,-18" viewBox="-40 -30 80 60"><path d="M-40-18h15M-40 18h15M25-18h15M-25-25h50v50h-50zM-25-24l8 6-8 6" fill="white" stroke="#172033" stroke-width="2"/><text x="-18" y="22" font-size="10">D</text><text x="13" y="-12" font-size="10">Q</text></symbol>
</defs>
"##;
use super::*;
