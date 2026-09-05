use super::*;

/// Compact larger schematics around their multi-port signal spine.
pub(super) fn compact_topology(items: &mut [Placed<'_>], roots: &BTreeMap<u64, u64>, spine_y: f64) {
    let anchors = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.component.ports.len() >= 3)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if anchors.len() < 2 || items.len() < 6 {
        return;
    }

    let rail_roots = items
        .iter()
        .filter(|item| item.component.kind == "ground")
        .flat_map(|item| item.component.ports.values())
        .map(|node| roots.get(node).copied().unwrap_or(*node))
        .collect::<BTreeSet<_>>();
    let raw_nets = items
        .iter()
        .map(|item| {
            item.component
                .ports
                .values()
                .map(|node| roots.get(node).copied().unwrap_or(*node))
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    // Shared ground is a rail, not a signal-flow edge. Including it makes every
    // grounded load appear adjacent to every active device and destroys ranks.
    let nets = raw_nets
        .iter()
        .map(|component_nets| {
            component_nets
                .difference(&rail_roots)
                .copied()
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    let connected = |a: usize, b: usize| !nets[a].is_disjoint(&nets[b]);
    let connected_raw = |a: usize, b: usize| !raw_nets[a].is_disjoint(&raw_nets[b]);

    for (column, index) in anchors.iter().copied().enumerate() {
        items[index].center = Point {
            x: 390.0 + column as f64 * 190.0,
            y: spine_y,
        };
        items[index].ports = port_points(
            items[index].component,
            items[index].center,
            items[index].orientation,
        );
    }

    let mut distance = vec![usize::MAX; items.len()];
    let mut owner = vec![usize::MAX; items.len()];
    let mut side = vec![-1_i8; items.len()];
    let mut queue = Vec::new();
    for anchor in anchors.iter().copied() {
        distance[anchor] = 0;
        owner[anchor] = anchor;
        queue.push(anchor);
    }
    let mut cursor = 0;
    while cursor < queue.len() {
        let current = queue[cursor];
        cursor += 1;
        for candidate in 0..items.len() {
            if distance[candidate] != usize::MAX || !connected(current, candidate) {
                continue;
            }
            distance[candidate] = distance[current] + 1;
            owner[candidate] = owner[current];
            side[candidate] = if distance[current] == 0 {
                let shared = nets[current].intersection(&nets[candidate]).next().copied();
                shared
                    .and_then(|root| {
                        items[current]
                            .component
                            .ports
                            .iter()
                            .find_map(|(name, node)| {
                                (roots.get(node).copied().unwrap_or(*node) == root).then(|| {
                                    let port = port_point(
                                        items[current].component,
                                        name,
                                        0,
                                        items[current].component.ports.len(),
                                        Point { x: 0.0, y: 0.0 },
                                        items[current].orientation,
                                    );
                                    if port.x > 0.0 { 1 } else { -1 }
                                })
                            })
                    })
                    .unwrap_or(-1)
            } else {
                side[current]
            };
            queue.push(candidate);
        }
    }

    let mut lanes = BTreeMap::<(usize, i8, usize), usize>::new();
    for index in 0..items.len() {
        if anchors.contains(&index)
            || matches!(
                items[index].orientation,
                Orientation::Ground
                    | Orientation::Supply
                    | Orientation::AnchorSupply
                    | Orientation::AnchorGround
            )
            || owner[index] == usize::MAX
        {
            continue;
        }
        let depth = distance[index].max(1);
        let lane = lanes.entry((owner[index], side[index], depth)).or_default();
        let anchor = items[owner[index]].center;
        items[index].center = Point {
            x: anchor.x + side[index] as f64 * (130.0 + (depth - 1) as f64 * 115.0),
            y: spine_y + 105.0 + *lane as f64 * 115.0,
        };
        let connected_anchors = anchors
            .iter()
            .copied()
            .filter(|anchor_index| connected(index, *anchor_index))
            .collect::<Vec<_>>();
        if depth == 1 && connected_anchors.len() >= 2 {
            let left = connected_anchors
                .iter()
                .map(|anchor_index| items[*anchor_index].center.x)
                .reduce(f64::min)
                .unwrap();
            let right = connected_anchors
                .iter()
                .map(|anchor_index| items[*anchor_index].center.x)
                .reduce(f64::max)
                .unwrap();
            items[index].center = Point {
                x: (left + right) / 2.0,
                y: spine_y + 115.0,
            };
        } else if depth == 1
            && connected_anchors.len() == 1
            && items[index].orientation == Orientation::Horizontal
            && items[index].component.ports.len() <= 2
        {
            let anchor_index = connected_anchors[0];
            let shared_root = nets[anchor_index]
                .intersection(&nets[index])
                .next()
                .copied();
            if let Some(root) = shared_root {
                let anchor_port = items[anchor_index]
                    .component
                    .ports
                    .iter()
                    .enumerate()
                    .find_map(|(port_index, (name, node))| {
                        (roots.get(node).copied().unwrap_or(*node) == root).then(|| {
                            port_point(
                                items[anchor_index].component,
                                name,
                                port_index,
                                items[anchor_index].component.ports.len(),
                                items[anchor_index].center,
                                items[anchor_index].orientation,
                            )
                        })
                    });
                let branch_offset = items[index].component.ports.iter().enumerate().find_map(
                    |(port_index, (name, node))| {
                        (roots.get(node).copied().unwrap_or(*node) == root).then(|| {
                            port_point(
                                items[index].component,
                                name,
                                port_index,
                                items[index].component.ports.len(),
                                Point { x: 0.0, y: 0.0 },
                                items[index].orientation,
                            )
                        })
                    },
                );
                if let (Some(anchor_port), Some(branch_offset)) = (anchor_port, branch_offset) {
                    items[index].center = Point {
                        x: anchor_port.x - branch_offset.x + side[index] as f64 * WIRE_LENGTH,
                        y: anchor_port.y - branch_offset.y,
                    };
                }
            }
        }
        *lane += 1;
        items[index].ports = port_points(
            items[index].component,
            items[index].center,
            items[index].orientation,
        );
    }

    // Sources sit directly above the nearest branch they feed. Grounds are
    // terminals below the leftmost grounded branch, not graph columns.
    for index in 0..items.len() {
        if items[index].orientation == Orientation::Supply {
            if let Some(neighbor) = (0..items.len())
                .filter(|candidate| *candidate != index && connected(index, *candidate))
                .min_by_key(|candidate| distance[*candidate])
            {
                items[index].center = Point {
                    x: items[neighbor].center.x,
                    y: items[neighbor].center.y - 120.0,
                };
            }
        } else if items[index].orientation == Orientation::Ground
            && let Some(neighbor) = (0..items.len())
                .filter(|candidate| {
                    *candidate != index
                        && connected_raw(index, *candidate)
                        && items[*candidate].component.ports.len() <= 2
                })
                .min_by(|a, b| items[*a].center.x.total_cmp(&items[*b].center.x))
        {
            items[index].center = Point {
                x: items[neighbor].center.x,
                y: items[neighbor].center.y + 105.0,
            };
        }
        items[index].ports = port_points(
            items[index].component,
            items[index].center,
            items[index].orientation,
        );
    }
}

pub(super) fn section_names(circuit: &CircuitDesign) -> Vec<String> {
    circuit.section_names()
}

pub(super) fn graph_layers(components: &[&Component], roots: &BTreeMap<u64, u64>) -> Vec<usize> {
    if components.is_empty() {
        return Vec::new();
    }
    let mut root_usage = BTreeMap::<u64, usize>::new();
    for component in components {
        for root in component
            .ports
            .values()
            .map(|node| roots.get(node).copied().unwrap_or(*node))
            .collect::<BTreeSet<_>>()
        {
            *root_usage.entry(root).or_default() += 1;
        }
    }
    let rail_roots = components
        .iter()
        .filter(|component| component.kind == "ground")
        .flat_map(|component| component.ports.values())
        .map(|node| roots.get(node).copied().unwrap_or(*node))
        .chain(
            root_usage
                .iter()
                .filter_map(|(root, usage)| (*usage >= 3).then_some(*root)),
        )
        .collect::<BTreeSet<_>>();
    let nets = components
        .iter()
        .map(|component| {
            component
                .ports
                .values()
                .map(|node| roots.get(node).copied().unwrap_or(*node))
                .filter(|root| !rail_roots.contains(root))
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    // Component declarations normally follow signal flow. Using the longest
    // connection to an earlier declaration preserves that flow even in a loop;
    // shortest-path BFS collapses both sides of a source into the same column.
    let mut layers = vec![0; components.len()];
    for index in 0..components.len() {
        let previous = (0..index)
            .filter(|other| !nets[index].is_disjoint(&nets[*other]))
            .map(|other| layers[other])
            .max();
        layers[index] = match previous {
            Some(layer) if components[index].kind == "ground" => layer,
            Some(layer) => layer + 1,
            // Independent branches belong in rows, not in an ever-deeper
            // diagonal merely because they were declared later.
            None => 0,
        };
    }
    layers
}

pub(super) fn orientation(
    component: &Component,
    placed_components: &[&Component],
    roots: &BTreeMap<u64, u64>,
) -> Orientation {
    match component.kind.as_str() {
        "ground" => Orientation::Ground,
        "voltage-source"
            if placed_components.iter().any(|candidate| {
                candidate.ports.len() >= 3 && components_share_net(component, candidate, roots)
            }) =>
        {
            if component
                .value
                .as_deref()
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<f64>().ok())
                == Some(0.0)
            {
                Orientation::AnchorGround
            } else {
                Orientation::AnchorSupply
            }
        }
        "voltage-source" => Orientation::Supply,
        "resistor" | "capacitor" | "inductor" | "switch" | "diode" | "led"
            if component.ports.len() <= 2 =>
        {
            if feeds_major_input(component, placed_components, roots) {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            }
        }
        _ => Orientation::Horizontal,
    }
}

pub(super) fn feeds_major_input(
    component: &Component,
    components: &[&Component],
    roots: &BTreeMap<u64, u64>,
) -> bool {
    let ground_roots = components
        .iter()
        .filter(|candidate| candidate.kind == "ground")
        .flat_map(|candidate| candidate.ports.values())
        .map(|node| roots.get(node).copied().unwrap_or(*node))
        .collect::<BTreeSet<_>>();
    let component_roots = component
        .ports
        .values()
        .map(|node| roots.get(node).copied().unwrap_or(*node))
        .collect::<BTreeSet<_>>();

    let mut reaches_input = false;
    let mut reaches_output = false;
    for candidate in components
        .iter()
        .filter(|candidate| candidate.ports.len() >= 3)
    {
        for (index, (name, node)) in candidate.ports.iter().enumerate() {
            let root = roots.get(node).copied().unwrap_or(*node);
            if ground_roots.contains(&root) || !component_roots.contains(&root) {
                continue;
            }
            let x = port_point(
                candidate,
                name,
                index,
                candidate.ports.len(),
                Point { x: 0.0, y: 0.0 },
                Orientation::Horizontal,
            )
            .x;
            reaches_input |= x < 0.0;
            reaches_output |= x > 0.0;
        }
    }
    reaches_input && !reaches_output
}

pub(super) fn components_share_net(
    a: &Component,
    b: &Component,
    roots: &BTreeMap<u64, u64>,
) -> bool {
    a.ports.values().any(|a_node| {
        let a_root = roots.get(a_node).copied().unwrap_or(*a_node);
        b.ports
            .values()
            .any(|b_node| roots.get(b_node).copied().unwrap_or(*b_node) == a_root)
    })
}

pub(super) fn align_anchor_feeders(
    placed: &mut [Placed<'_>],
    section: &str,
    roots: &BTreeMap<u64, u64>,
) {
    let anchors = placed
        .iter()
        .filter(|item| item.section == section && item.component.ports.len() >= 3)
        .map(|item| {
            let ports = item
                .component
                .ports
                .values()
                .filter_map(|node| {
                    item.ports
                        .get(node)
                        .map(|point| (roots.get(node).copied().unwrap_or(*node), *point))
                })
                .collect::<Vec<_>>();
            (item.center, ports)
        })
        .collect::<Vec<_>>();

    let mut candidates = Vec::new();
    for (item_index, item) in placed.iter().enumerate().filter(|(_, item)| {
        item.section == section
            && matches!(
                item.orientation,
                Orientation::AnchorSupply | Orientation::AnchorGround
            )
    }) {
        let source_roots = item
            .component
            .ports
            .values()
            .map(|node| roots.get(node).copied().unwrap_or(*node))
            .collect::<BTreeSet<_>>();
        if let Some((anchor_index, anchor, pin)) =
            anchors
                .iter()
                .enumerate()
                .find_map(|(anchor_index, (anchor, ports))| {
                    ports
                        .iter()
                        .find(|(root, _)| source_roots.contains(root))
                        .map(|(_, point)| (anchor_index, *anchor, *point))
                })
        {
            candidates.push((
                item_index,
                anchor_index,
                anchor,
                pin,
                item.orientation == Orientation::AnchorGround,
            ));
        }
    }

    // Inner lanes serve the nearest outside-facing pin. This produces nested
    // feeder routes instead of making declaration order decide whether two
    // independent rails cross one another.
    candidates.sort_by(|a, b| {
        (a.1, a.4)
            .cmp(&(b.1, b.4))
            .then_with(|| {
                if a.4 {
                    b.3.y.total_cmp(&a.3.y)
                } else {
                    a.3.y.total_cmp(&b.3.y)
                }
            })
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut lane_counts = BTreeMap::<(usize, bool), usize>::new();
    for (item_index, anchor_index, anchor, pin, downward) in candidates {
        let lane = lane_counts.entry((anchor_index, downward)).or_default();
        let item = &mut placed[item_index];
        item.center.x = pin.x - WIRE_LENGTH - *lane as f64 * (SYMBOL_WIDTH + 20.0);
        item.center.y = match item.orientation {
            Orientation::AnchorSupply => anchor.y - WIRE_LENGTH - 30.0,
            Orientation::AnchorGround => {
                let terminal = svg_port(STYLE_AND_SYMBOLS, "symbol-ground", "node")
                    .map(|(_, y)| y.abs())
                    .unwrap_or(25.0);
                anchor.y + WIRE_LENGTH + terminal
            }
            _ => unreachable!(),
        };
        item.ports = port_points(item.component, item.center, item.orientation);
        *lane += 1;
    }
}

pub(super) fn port_points(
    component: &Component,
    center: Point,
    orientation: Orientation,
) -> PortPositions {
    let count = component.ports.len();
    component
        .ports
        .iter()
        .enumerate()
        .map(|(index, (name, node))| {
            let point = port_point(component, name, index, count, center, orientation);
            (name.clone(), (*node, point))
        })
        .collect()
}

pub(super) fn port_point(
    component: &Component,
    name: &str,
    index: usize,
    count: usize,
    center: Point,
    orientation: Orientation,
) -> Point {
    let symbol = match orientation {
        Orientation::AnchorSupply => "symbol-voltage-source".to_owned(),
        Orientation::AnchorGround => "symbol-ground".to_owned(),
        _ if component.svg.is_some() => String::new(),
        _ => symbol_id(&component.kind),
    };
    let metadata = component.svg.as_deref().unwrap_or(STYLE_AND_SYMBOLS);
    let metadata_name = if orientation == Orientation::AnchorGround && name == "positive" {
        "node"
    } else {
        name
    };
    let (mut x, mut y) = if is_generic_component(component) {
        generic_port_offset(component, index, count)
    } else {
        svg_port(metadata, &symbol, metadata_name)
            .unwrap_or_else(|| generic_port_offset(component, index, count))
    };
    if orientation == Orientation::Vertical {
        (x, y) = (-y, x);
    }
    Point {
        x: center.x + x,
        y: center.y + y,
    }
}

pub(super) fn is_generic_component(component: &Component) -> bool {
    component.svg.is_none() && symbol_id(&component.kind) == "symbol-generic"
}

/// Generic ICs used to be squeezed into the same 80x60 tile as a resistor.
/// Give them a body sized for their pin count and split pins over both sides.
pub(super) fn component_size(component: &Component) -> (f64, f64) {
    if !is_generic_component(component) {
        return (SYMBOL_WIDTH, SYMBOL_HEIGHT);
    }
    let longest = component
        .ports
        .keys()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(1) as f64;
    let width = (130.0 + longest * 5.5).clamp(150.0, 240.0);
    let rows = component.ports.len().div_ceil(2).max(1) as f64;
    (width, (rows * 22.0 + 28.0).max(64.0))
}

pub(super) fn generic_port_offset(component: &Component, index: usize, count: usize) -> (f64, f64) {
    let (width, height) = component_size(component);
    let left_count = count.div_ceil(2);
    let (side, row, side_count) = if index < left_count {
        (-1.0, index, left_count)
    } else {
        (1.0, index - left_count, count - left_count)
    };
    let y = (row as f64 - side_count.saturating_sub(1) as f64 / 2.0) * 22.0;
    (
        side * width / 2.0,
        y.clamp(-height / 2.0 + 12.0, height / 2.0 - 12.0),
    )
}

/// Reads `name:x,y` entries from a symbol's `data-ports` SVG metadata.
pub(super) fn svg_port(svg: &str, symbol: &str, name: &str) -> Option<(f64, f64)> {
    let scope = if symbol.is_empty() {
        svg
    } else {
        let marker = format!(r#"<symbol id="{symbol}""#);
        &svg[svg.find(&marker)?..]
    };
    let opening_tag = &scope[..scope.find('>')?];
    let marker = r#"data-ports=""#;
    let ports = &opening_tag[opening_tag.find(marker)? + marker.len()..];
    let ports = &ports[..ports.find('"')?];
    ports.split(';').find_map(|entry| {
        let (port_name, coordinates) = entry.split_once(':')?;
        if port_name != name {
            return None;
        }
        let (x, y) = coordinates.split_once(',')?;
        Some((x.parse().ok()?, y.parse().ok()?))
    })
}
