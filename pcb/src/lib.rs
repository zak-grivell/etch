use std::collections::BTreeMap;
use std::fmt::Write;

use circuit_ir::{CircuitDesign, Component, PcbConfig};
use render_utils::escape_xml;

#[derive(Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub struct PlacedComponent {
    pub component_index: usize,
    pub center: Point,
    pub width: f64,
    pub height: f64,
    pub pads: BTreeMap<u64, Point>,
}

pub struct PcbLayout {
    pub components: Vec<PlacedComponent>,
    pub nets: BTreeMap<u64, Vec<Point>>,
    pub traces: Vec<TraceSegment>,
}

pub struct TraceSegment {
    pub root: u64,
    pub layer: usize,
    pub points: [Point; 3],
}

/// Produce a deliberately small, deterministic first PCB implementation.
///
/// Its data boundary mirrors tscircuit's SimpleRouteJson: board bounds and
/// layer count come from `pcb_config`, symbols become rectangular obstacles,
/// and each electrical root becomes a connection between pad points. The
/// initial router uses orthogonal paths and distributes nets between layers.
pub fn render(circuit: &CircuitDesign) -> Result<String, String> {
    let config = circuit
        .pcb
        .as_ref()
        .ok_or_else(|| "PCB generation requires pcb_config(...)".to_owned())?;
    let layout = layout(circuit)?;
    Ok(draw(circuit, config, &layout))
}

pub fn layout(circuit: &CircuitDesign) -> Result<PcbLayout, String> {
    let config = circuit
        .pcb
        .as_ref()
        .ok_or_else(|| "PCB generation requires pcb_config(...)".to_owned())?;
    let roots = circuit.electrical_roots();
    let components = circuit
        .components
        .iter()
        .enumerate()
        .filter(|(_, component)| !matches!(component.kind.as_str(), "ground" | "supply"))
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Err("PCB generation requires at least one component symbol".into());
    }
    let placed = place_components(&components, config)?;
    let nets = collect_nets(&placed, &roots);
    let traces = route_nets(&nets, config);
    Ok(PcbLayout {
        components: placed,
        nets,
        traces,
    })
}

fn route_nets(nets: &BTreeMap<u64, Vec<Point>>, config: &PcbConfig) -> Vec<TraceSegment> {
    let mut traces = Vec::new();
    for (net_index, (root, points)) in nets.iter().enumerate() {
        let layer_offset = (net_index % config.layers) as f64 * config.clearance;
        let mut previous = points[0];
        for target in points.iter().copied().skip(1) {
            let bend = if net_index % 2 == 0 {
                Point {
                    x: target.x,
                    y: previous.y + layer_offset,
                }
            } else {
                Point {
                    x: previous.x + layer_offset,
                    y: target.y,
                }
            };
            traces.push(TraceSegment {
                root: *root,
                layer: net_index % config.layers,
                points: [previous, bend, target],
            });
            previous = target;
        }
    }
    traces
}

fn place_components(
    components: &[(usize, &Component)],
    config: &PcbConfig,
) -> Result<Vec<PlacedComponent>, String> {
    const BODY_WIDTH: f64 = 5.0;
    const PAD_PITCH: f64 = 1.6;
    let margin = 3.0;
    let columns = ((config.width - margin * 2.0) / 9.0).floor().max(1.0) as usize;
    let mut placed = Vec::with_capacity(components.len());
    for (index, (component_index, component)) in components.iter().enumerate() {
        let port_count = component.ports.len().max(1);
        let height = (port_count.div_ceil(2) as f64 * PAD_PITCH + 1.8).max(4.0);
        let column = index % columns;
        let row = index / columns;
        let center = Point {
            x: -config.width / 2.0 + margin + BODY_WIDTH / 2.0 + column as f64 * 9.0,
            y: config.height / 2.0 - margin - height / 2.0 - row as f64 * 8.0,
        };
        if center.y - height / 2.0 < -config.height / 2.0 + margin {
            return Err(format!(
                "{} components do not fit on a {} mm by {} mm board",
                components.len(),
                config.width,
                config.height
            ));
        }
        let mut pads = BTreeMap::new();
        for (pad_index, node) in component.ports.values().enumerate() {
            let left = pad_index % 2 == 0;
            let side_index = pad_index / 2;
            let side_count = if left {
                port_count.div_ceil(2)
            } else {
                port_count / 2
            };
            let y = center.y + (side_count.saturating_sub(1) as f64 * PAD_PITCH / 2.0)
                - side_index as f64 * PAD_PITCH;
            pads.insert(
                *node,
                Point {
                    x: center.x
                        + if left {
                            -BODY_WIDTH / 2.0
                        } else {
                            BODY_WIDTH / 2.0
                        },
                    y,
                },
            );
        }
        placed.push(PlacedComponent {
            component_index: *component_index,
            center,
            width: BODY_WIDTH,
            height,
            pads,
        });
    }
    Ok(placed)
}

fn collect_nets(
    placed: &[PlacedComponent],
    roots: &BTreeMap<u64, u64>,
) -> BTreeMap<u64, Vec<Point>> {
    let mut nets = BTreeMap::<u64, Vec<Point>>::new();
    for item in placed {
        for (node, point) in &item.pads {
            nets.entry(roots.get(node).copied().unwrap_or(*node))
                .or_default()
                .push(*point);
        }
    }
    nets.retain(|_, points| points.len() > 1);
    nets
}

fn draw(circuit: &CircuitDesign, config: &PcbConfig, layout: &PcbLayout) -> String {
    const SCALE: f64 = 18.0;
    const BORDER: f64 = 24.0;
    let width = config.width * SCALE + BORDER * 2.0;
    let height = config.height * SCALE + BORDER * 2.0;
    let map = |point: Point| Point {
        x: BORDER + (point.x + config.width / 2.0) * SCALE,
        y: BORDER + (config.height / 2.0 - point.y) * SCALE,
    };
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    )
    .unwrap();
    svg.push_str(r##"<rect width="100%" height="100%" fill="#111827"/>"##);
    writeln!(
        svg,
        r##"<rect x="{BORDER}" y="{BORDER}" width="{}" height="{}" rx="5" fill="#166534" stroke="#d1fae5" stroke-width="2"/>"##,
        config.width * SCALE,
        config.height * SCALE
    )
    .unwrap();

    let colors = ["#fbbf24", "#38bdf8", "#fb7185", "#c084fc"];
    for trace in &layout.traces {
        let color = colors[trace.layer % colors.len()];
        let [a, b, c] = trace.points.map(map);
        writeln!(
            svg,
            r#"<path d="M {} {} L {} {} L {} {}" fill="none" stroke="{color}" stroke-width="{}" stroke-linejoin="round" opacity="0.9"/>"#,
            a.x,
            a.y,
            b.x,
            b.y,
            c.x,
            c.y,
            (config.min_trace_width * SCALE).max(1.5)
        )
        .unwrap();
    }

    for (index, item) in layout.components.iter().enumerate() {
        let component = &circuit.components[item.component_index];
        let center = map(item.center);
        writeln!(
            svg,
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="2" fill="#1f2937" stroke="#f8fafc" stroke-width="1.5"/>"##,
            center.x - item.width * SCALE / 2.0,
            center.y - item.height * SCALE / 2.0,
            item.width * SCALE,
            item.height * SCALE
        )
        .unwrap();
        for point in item.pads.values() {
            let point = map(*point);
            writeln!(
                svg,
                r##"<circle cx="{}" cy="{}" r="4" fill="#fbbf24" stroke="#111827"/>"##,
                point.x, point.y
            )
            .unwrap();
        }
        let label = component
            .label
            .as_deref()
            .unwrap_or(component.kind.as_str());
        writeln!(
            svg,
            r#"<text x="{}" y="{}" fill="white" font-family="monospace" font-size="11" text-anchor="middle">{}</text>"#,
            center.x,
            center.y + 4.0,
            escape_xml(label)
        )
        .unwrap();
        let _ = index;
    }
    writeln!(
        svg,
        r##"<text x="{BORDER}" y="16" fill="#cbd5e1" font-family="sans-serif" font-size="11">{} × {} mm · {} layer(s) · {} routed net(s)</text>"##,
        config.width,
        config.height,
        config.layers,
        layout.nets.len()
    )
    .unwrap();
    let _ = circuit;
    svg.push_str("</svg>\n");
    svg
}
