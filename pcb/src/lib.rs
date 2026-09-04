use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
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
    pub rotation: i32,
    pub pads: BTreeMap<u64, Point>,
}

pub struct PcbLayout {
    pub components: Vec<PlacedComponent>,
    pub nets: BTreeMap<u64, Vec<Point>>,
    pub traces: Vec<TraceSegment>,
    pub unrouted_connections: usize,
}

pub struct TraceSegment {
    pub root: u64,
    pub layer: usize,
    pub points: Vec<Point>,
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
    let placed = place_components(&components, config, &roots)?;
    let nets = collect_nets(&placed, &roots);
    let traces = route_nets(&nets, &placed, &roots, config);
    let required_connections = nets.values().map(|points| points.len() - 1).sum::<usize>();
    let unrouted_connections = required_connections.saturating_sub(traces.len());
    Ok(PcbLayout {
        components: placed,
        nets,
        traces,
        unrouted_connections,
    })
}

#[derive(Clone, Copy)]
struct Hitbox {
    left: f64,
    right: f64,
    bottom: f64,
    top: f64,
}

fn route_nets(
    nets: &BTreeMap<u64, Vec<Point>>,
    placed: &[PlacedComponent],
    roots: &BTreeMap<u64, u64>,
    config: &PcbConfig,
) -> Vec<TraceSegment> {
    let mut traces = Vec::new();
    let component_obstacles = placed
        .iter()
        .map(|item| Hitbox {
            left: item.center.x - item.width / 2.0,
            right: item.center.x + item.width / 2.0,
            bottom: item.center.y - item.height / 2.0,
            top: item.center.y + item.height / 2.0,
        })
        .collect::<Vec<_>>();
    let pad_obstacles = placed
        .iter()
        .flat_map(|item| {
            item.pads.iter().map(|(node, point)| {
                let root = roots.get(node).copied().unwrap_or(*node);
                (root, *point)
            })
        })
        .collect::<Vec<_>>();
    let layer_count = config.layers.clamp(1, 32);
    let mut occupied = vec![Vec::<(u64, Hitbox)>::new(); layer_count];
    for (root, points) in nets {
        let mut tree = vec![points[0]];
        let mut remaining = points.iter().copied().skip(1).collect::<Vec<_>>();
        while !remaining.is_empty() {
            let (remaining_index, target, tree_point) = remaining
                .iter()
                .enumerate()
                .flat_map(|(index, target)| tree.iter().map(move |tree| (index, *target, *tree)))
                .min_by(|a, b| manhattan(a.1, a.2).total_cmp(&manhattan(b.1, b.2)))
                .unwrap();
            let mut layers = (0..layer_count).collect::<Vec<_>>();
            layers.sort_by_key(|layer| occupied[*layer].len());
            let mut best = None;
            for layer in layers {
                let mut obstacles = component_obstacles
                    .iter()
                    .copied()
                    .filter(|obstacle| {
                        !contains(*obstacle, target, 0.01) && !contains(*obstacle, tree_point, 0.01)
                    })
                    .map(|obstacle| {
                        inflate(obstacle, config.clearance + config.min_trace_width / 2.0)
                    })
                    .collect::<Vec<_>>();
                obstacles.extend(
                    occupied[layer]
                        .iter()
                        .filter(|(other_root, _)| other_root != root)
                        .map(|(_, obstacle)| *obstacle),
                );
                obstacles.extend(
                    pad_obstacles
                        .iter()
                        .filter(|(pad_root, _)| pad_root != root)
                        .map(|(_, point)| Hitbox {
                            left: point.x - 0.6 - config.clearance - config.min_trace_width / 2.0,
                            right: point.x + 0.6 + config.clearance + config.min_trace_width / 2.0,
                            bottom: point.y - 0.6 - config.clearance - config.min_trace_width / 2.0,
                            top: point.y + 0.6 + config.clearance + config.min_trace_width / 2.0,
                        }),
                );
                if let Some(path) = route_between(target, tree_point, &obstacles, config) {
                    best = Some((layer, path));
                    break;
                }
            }
            let Some((layer, path)) = best else {
                remaining.remove(remaining_index);
                continue;
            };
            for pair in path.windows(2) {
                occupied[layer].push((
                    *root,
                    segment_hitbox(pair[0], pair[1], config.clearance + config.min_trace_width),
                ));
            }
            traces.push(TraceSegment {
                root: *root,
                layer,
                points: path.clone(),
            });
            tree.extend(path);
            remaining.remove(remaining_index);
        }
    }
    traces
}

fn route_between(
    start: Point,
    goal: Point,
    obstacles: &[Hitbox],
    config: &PcbConfig,
) -> Option<Vec<Point>> {
    for corner in [
        Point {
            x: goal.x,
            y: start.y,
        },
        Point {
            x: start.x,
            y: goal.y,
        },
    ] {
        let path = simplify(vec![start, corner, goal]);
        if path
            .windows(2)
            .all(|pair| segment_clear(pair[0], pair[1], obstacles))
        {
            return Some(path);
        }
    }
    let channel = (config.clearance + config.min_trace_width).max(0.25);
    let mut xs = vec![
        start.x,
        goal.x,
        -config.width / 2.0 + channel,
        config.width / 2.0 - channel,
    ];
    let mut ys = vec![
        start.y,
        goal.y,
        -config.height / 2.0 + channel,
        config.height / 2.0 - channel,
    ];
    for obstacle in obstacles {
        xs.extend([obstacle.left - channel, obstacle.right + channel]);
        ys.extend([obstacle.bottom - channel, obstacle.top + channel]);
    }
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    let index_of = |point: Point| {
        let x = xs
            .iter()
            .position(|value| (*value - point.x).abs() < 0.001)?;
        let y = ys
            .iter()
            .position(|value| (*value - point.y).abs() < 0.001)?;
        Some(y * xs.len() + x)
    };
    let start_index = index_of(start)?;
    let goal_index = index_of(goal)?;
    let mut distance = vec![i64::MAX; xs.len() * ys.len()];
    let mut previous = vec![None; distance.len()];
    let mut queue = BinaryHeap::new();
    distance[start_index] = 0;
    queue.push((Reverse(0_i64), start_index));
    while let Some((Reverse(cost), node)) = queue.pop() {
        if node == goal_index {
            break;
        }
        if cost != distance[node] {
            continue;
        }
        let xi = node % xs.len();
        let yi = node / xs.len();
        let neighbors = [
            (xi > 0).then_some(node.wrapping_sub(1)),
            (xi + 1 < xs.len()).then_some(node + 1),
            (yi > 0).then_some(node.wrapping_sub(xs.len())),
            (yi + 1 < ys.len()).then_some(node + xs.len()),
        ];
        let from = Point {
            x: xs[xi],
            y: ys[yi],
        };
        for next in neighbors.into_iter().flatten() {
            let to = Point {
                x: xs[next % xs.len()],
                y: ys[next / xs.len()],
            };
            if !segment_clear(from, to, obstacles) {
                continue;
            }
            let next_cost = cost + (manhattan(from, to) * 1000.0).round() as i64;
            if next_cost < distance[next] {
                distance[next] = next_cost;
                previous[next] = Some(node);
                queue.push((Reverse(next_cost), next));
            }
        }
    }
    if distance[goal_index] == i64::MAX {
        return None;
    }
    let mut nodes = vec![goal_index];
    while *nodes.last().unwrap() != start_index {
        nodes.push(previous[*nodes.last().unwrap()]?);
    }
    nodes.reverse();
    Some(simplify(
        nodes
            .into_iter()
            .map(|node| Point {
                x: xs[node % xs.len()],
                y: ys[node / xs.len()],
            })
            .collect(),
    ))
}

fn segment_clear(a: Point, b: Point, obstacles: &[Hitbox]) -> bool {
    obstacles.iter().all(|obstacle| {
        if (a.y - b.y).abs() < 0.001 {
            !(a.y > obstacle.bottom
                && a.y < obstacle.top
                && a.x.max(b.x) > obstacle.left
                && a.x.min(b.x) < obstacle.right)
        } else {
            !(a.x > obstacle.left
                && a.x < obstacle.right
                && a.y.max(b.y) > obstacle.bottom
                && a.y.min(b.y) < obstacle.top)
        }
    })
}

fn simplify(path: Vec<Point>) -> Vec<Point> {
    let mut result: Vec<Point> = Vec::new();
    for point in path {
        if result.len() >= 2 {
            let a = result[result.len() - 2];
            let b = result[result.len() - 1];
            if ((a.x - b.x).abs() < 0.001 && (b.x - point.x).abs() < 0.001)
                || ((a.y - b.y).abs() < 0.001 && (b.y - point.y).abs() < 0.001)
            {
                result.pop();
            }
        }
        if result
            .last()
            .is_none_or(|last| manhattan(*last, point) > 0.001)
        {
            result.push(point);
        }
    }
    result
}

fn contains(box_: Hitbox, point: Point, epsilon: f64) -> bool {
    point.x >= box_.left - epsilon
        && point.x <= box_.right + epsilon
        && point.y >= box_.bottom - epsilon
        && point.y <= box_.top + epsilon
}

fn inflate(box_: Hitbox, amount: f64) -> Hitbox {
    Hitbox {
        left: box_.left - amount,
        right: box_.right + amount,
        bottom: box_.bottom - amount,
        top: box_.top + amount,
    }
}

fn segment_hitbox(a: Point, b: Point, amount: f64) -> Hitbox {
    Hitbox {
        left: a.x.min(b.x) - amount,
        right: a.x.max(b.x) + amount,
        bottom: a.y.min(b.y) - amount,
        top: a.y.max(b.y) + amount,
    }
}

fn place_components(
    components: &[(usize, &Component)],
    config: &PcbConfig,
    roots: &BTreeMap<u64, u64>,
) -> Result<Vec<PlacedComponent>, String> {
    const BODY_WIDTH: f64 = 5.0;
    const PAD_PITCH: f64 = 1.6;
    const MARGIN: f64 = 2.0;
    let mut order = components
        .iter()
        .map(|(component_index, component)| {
            let port_count = component.ports.len().max(1);
            let height = (port_count.div_ceil(2) as f64 * PAD_PITCH + 1.8).max(4.0);
            (*component_index, *component, BODY_WIDTH * height)
        })
        .collect::<Vec<_>>();
    order.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    let mut placed: Vec<PlacedComponent> = Vec::with_capacity(components.len());
    for (component_index, component, _) in order {
        let port_count = component.ports.len().max(1);
        let natural_height = (port_count.div_ceil(2) as f64 * PAD_PITCH + 1.8).max(4.0);
        let mut best: Option<(f64, PlacedComponent)> = None;
        for rotation in [0, 90, 180, 270] {
            let (width, height) = if rotation % 180 == 0 {
                (BODY_WIDTH, natural_height)
            } else {
                (natural_height, BODY_WIDTH)
            };
            let min_x = -config.width / 2.0 + MARGIN + width / 2.0;
            let max_x = config.width / 2.0 - MARGIN - width / 2.0;
            let min_y = -config.height / 2.0 + MARGIN + height / 2.0;
            let max_y = config.height / 2.0 - MARGIN - height / 2.0;
            let mut candidates = vec![Point { x: 0.0, y: 0.0 }];
            for other in &placed {
                let gap = config.clearance;
                let left = other.center.x - other.width / 2.0 - gap - width / 2.0;
                let right = other.center.x + other.width / 2.0 + gap + width / 2.0;
                let below = other.center.y - other.height / 2.0 - gap - height / 2.0;
                let above = other.center.y + other.height / 2.0 + gap + height / 2.0;
                let aligned_x = [
                    other.center.x,
                    other.center.x - other.width / 2.0 + width / 2.0,
                    other.center.x + other.width / 2.0 - width / 2.0,
                ];
                let aligned_y = [
                    other.center.y,
                    other.center.y - other.height / 2.0 + height / 2.0,
                    other.center.y + other.height / 2.0 - height / 2.0,
                ];
                candidates.extend(
                    aligned_y
                        .into_iter()
                        .flat_map(|y| [Point { x: left, y }, Point { x: right, y }]),
                );
                candidates.extend(
                    aligned_x
                        .into_iter()
                        .flat_map(|x| [Point { x, y: below }, Point { x, y: above }]),
                );
            }
            // Completeness fallback for cavities not represented by the
            // rectangular packed outline. A coarse grid keeps this bounded.
            let fallback_grid = 3.0_f64.max(config.clearance + config.min_trace_width);
            let mut grid_y = min_y;
            while grid_y <= max_y + 0.001 {
                let mut grid_x = min_x;
                while grid_x <= max_x + 0.001 {
                    candidates.push(Point {
                        x: grid_x,
                        y: grid_y,
                    });
                    grid_x += fallback_grid;
                }
                grid_y += fallback_grid;
            }
            let mut evaluate = |center: Point| {
                if center.x < min_x || center.x > max_x || center.y < min_y || center.y > max_y {
                    return;
                }
                if placed.iter().any(|other| {
                    boxes_overlap(
                        center,
                        width,
                        height,
                        other.center,
                        other.width,
                        other.height,
                        config.clearance + 1.2,
                    )
                }) {
                    return;
                }
                let pads = component_pads(component, center, rotation);
                let mut connection_cost = 0.0;
                let mut connection_count = 0usize;
                for (node, pad) in &pads {
                    let root = roots.get(node).copied().unwrap_or(*node);
                    if let Some(distance) = placed
                        .iter()
                        .flat_map(|other| &other.pads)
                        .filter(|(other_node, _)| {
                            roots.get(other_node).copied().unwrap_or(**other_node) == root
                        })
                        .map(|(_, other_pad)| manhattan(*pad, *other_pad))
                        .reduce(f64::min)
                    {
                        connection_cost += distance;
                        connection_count += 1;
                    }
                }
                let center_cost = center.x.abs() + center.y.abs();
                let score = if connection_count == 0 {
                    center_cost
                } else {
                    connection_cost * 20.0 + center_cost
                };
                let candidate = PlacedComponent {
                    component_index,
                    center,
                    width,
                    height,
                    rotation,
                    pads,
                };
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score < *best_score)
                {
                    best = Some((score, candidate));
                }
            };
            for center in candidates {
                evaluate(center);
            }
        }
        let Some((_, candidate)) = best else {
            return Err(format!(
                "component `{}` does not fit on a {} mm by {} mm board with {} mm clearance",
                component.kind, config.width, config.height, config.clearance
            ));
        };
        placed.push(candidate);
    }
    placed.sort_by_key(|item| item.component_index);
    Ok(placed)
}

fn component_pads(component: &Component, center: Point, rotation: i32) -> BTreeMap<u64, Point> {
    const BODY_WIDTH: f64 = 5.0;
    const PAD_PITCH: f64 = 1.6;
    let port_count = component.ports.len().max(1);
    component
        .ports
        .values()
        .enumerate()
        .map(|(pad_index, node)| {
            let left = pad_index % 2 == 0;
            let side_index = pad_index / 2;
            let side_count = if left {
                port_count.div_ceil(2)
            } else {
                port_count / 2
            };
            let local = Point {
                x: if left {
                    -BODY_WIDTH / 2.0
                } else {
                    BODY_WIDTH / 2.0
                },
                y: (side_count.saturating_sub(1) as f64 * PAD_PITCH / 2.0)
                    - side_index as f64 * PAD_PITCH,
            };
            let rotated = rotate(local, rotation);
            (
                *node,
                Point {
                    x: center.x + rotated.x,
                    y: center.y + rotated.y,
                },
            )
        })
        .collect()
}

fn rotate(point: Point, rotation: i32) -> Point {
    match rotation.rem_euclid(360) {
        90 => Point {
            x: -point.y,
            y: point.x,
        },
        180 => Point {
            x: -point.x,
            y: -point.y,
        },
        270 => Point {
            x: point.y,
            y: -point.x,
        },
        _ => point,
    }
}

fn boxes_overlap(a: Point, aw: f64, ah: f64, b: Point, bw: f64, bh: f64, clearance: f64) -> bool {
    (a.x - b.x).abs() < (aw + bw) / 2.0 + clearance
        && (a.y - b.y).abs() < (ah + bh) / 2.0 + clearance
}

fn manhattan(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
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
        let points = trace.points.iter().copied().map(map).collect::<Vec<_>>();
        let Some(first) = points.first() else {
            continue;
        };
        let path =
            points
                .iter()
                .skip(1)
                .fold(format!("M {} {}", first.x, first.y), |mut path, point| {
                    write!(path, " L {} {}", point.x, point.y).unwrap();
                    path
                });
        writeln!(
            svg,
            r#"<path d="{path}" fill="none" stroke="{color}" stroke-width="{}" stroke-linejoin="round" opacity="0.9"/>"#,
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
        r##"<text x="{BORDER}" y="16" fill="#cbd5e1" font-family="sans-serif" font-size="11">{} × {} mm · {} layer(s) · {} routed connection(s) · {} unrouted</text>"##,
        config.width,
        config.height,
        config.layers,
        layout.traces.len(),
        layout.unrouted_connections
    )
    .unwrap();
    let _ = circuit;
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obstacle_router_does_not_cross_component_body() {
        let config = PcbConfig {
            width: 30.0,
            height: 20.0,
            layers: 2,
            min_trace_width: 0.2,
            clearance: 0.2,
        };
        let obstacle = Hitbox {
            left: -2.0,
            right: 2.0,
            bottom: -2.0,
            top: 2.0,
        };
        let path = route_between(
            Point { x: -8.0, y: 0.0 },
            Point { x: 8.0, y: 0.0 },
            &[obstacle],
            &config,
        )
        .unwrap();
        assert!(path.len() >= 4);
        assert!(
            path.windows(2)
                .all(|pair| segment_clear(pair[0], pair[1], &[obstacle]))
        );
    }

    #[test]
    fn component_clearance_is_symmetric() {
        assert!(boxes_overlap(
            Point { x: 0.0, y: 0.0 },
            4.0,
            4.0,
            Point { x: 4.1, y: 0.0 },
            4.0,
            4.0,
            0.2,
        ));
        assert!(!boxes_overlap(
            Point { x: 0.0, y: 0.0 },
            4.0,
            4.0,
            Point { x: 4.2, y: 0.0 },
            4.0,
            4.0,
            0.2,
        ));
    }
}
