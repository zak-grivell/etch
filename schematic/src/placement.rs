use super::*;

pub(super) fn resolve_component_overlaps(
    circuit: &CircuitDesign,
    placed: &mut [Placed<'_>],
    roots: &BTreeMap<u64, u64>,
) {
    const CLEARANCE: f64 = 16.0;
    for _ in 0..placed.len().saturating_mul(placed.len()).saturating_mul(4) {
        let mut collision = None;
        'search: for first in 0..placed.len() {
            for second in first + 1..placed.len() {
                if placed[first].section != placed[second].section {
                    continue;
                }
                if !component_is_visible(circuit, &placed[first], roots)
                    || !component_is_visible(circuit, &placed[second], roots)
                {
                    continue;
                }
                let attached_terminal = matches!(
                    placed[first].orientation,
                    Orientation::AnchorSupply | Orientation::AnchorGround
                ) || matches!(
                    placed[second].orientation,
                    Orientation::AnchorSupply | Orientation::AnchorGround
                );
                if attached_terminal
                    && components_share_net(
                        placed[first].component,
                        placed[second].component,
                        roots,
                    )
                {
                    continue;
                }
                let a = component_hitbox(&placed[first], CLEARANCE);
                let b = component_hitbox(&placed[second], CLEARANCE);
                if hitboxes_overlap(a, b) {
                    collision = Some((first, second));
                    break 'search;
                }
            }
        }
        let Some((first, second)) = collision else {
            return;
        };
        // Always move the lower item farther down. Choosing by component type
        // could move an upper item through an entire stack, causing the same
        // pairings to collide again and making large sections grow without
        // bound. Move by the exact overlap instead of a full grid row.
        let (move_index, fixed_index) = if placed[first].center.y > placed[second].center.y {
            (first, second)
        } else {
            (second, first)
        };
        let moving = component_hitbox(&placed[move_index], CLEARANCE);
        let fixed = component_hitbox(&placed[fixed_index], CLEARANCE);
        let offset = (fixed.bottom - moving.top + 1.0).max(1.0);
        placed[move_index].center.y += offset;
        for point in placed[move_index].ports.values_mut() {
            point.y += offset;
        }
    }
    debug_assert!(!placed.iter().enumerate().any(|(first, a)| {
        placed.iter().skip(first + 1).any(|b| {
            a.section == b.section
                && component_is_visible(circuit, a, roots)
                && component_is_visible(circuit, b, roots)
                && hitboxes_overlap(
                    component_hitbox(a, CLEARANCE),
                    component_hitbox(b, CLEARANCE),
                )
        })
    }));
}

pub(super) fn component_hitbox(item: &Placed<'_>, clearance: f64) -> Hitbox {
    let (component_width, component_height) = component_size(item.component);
    let (half_width, half_height, effective_clearance) = match item.orientation {
        // Rail glyphs occupy only the bar/stem described by their SVG metadata;
        // treating them as full component tiles creates false collisions that
        // push them away from the ports they are meant to feed.
        Orientation::AnchorSupply | Orientation::Supply => (30.0, 30.0, clearance.min(4.0)),
        Orientation::AnchorGround | Orientation::Ground => (22.0, 25.0, clearance.min(4.0)),
        _ if is_generic_component(item.component) => (
            component_width / 2.0,
            component_height / 2.0,
            clearance.min(10.0),
        ),
        _ if item.component.ports.len() >= 3 => (40.0, 25.0, clearance.min(8.0)),
        Orientation::Vertical => (component_height / 2.0, component_width / 2.0, clearance),
        _ => (component_width / 2.0, component_height / 2.0, clearance),
    };
    Hitbox {
        left: item.center.x - half_width - effective_clearance,
        right: item.center.x + half_width + effective_clearance,
        top: item.center.y - half_height - effective_clearance,
        bottom: item.center.y + half_height + effective_clearance,
    }
}

pub(super) fn routing_hitbox(item: &Placed<'_>, clearance: f64) -> Hitbox {
    let (component_width, component_height) = component_size(item.component);
    let value_width = item
        .component
        .value
        .as_deref()
        .map(|value| component_value(value).chars().count() as f64 * 7.2)
        .unwrap_or(0.0);
    let (left_extent, right_extent, top_extent, bottom_extent) = match item.orientation {
        Orientation::AnchorSupply | Orientation::Supply => (30.0, 30.0, 30.0, 30.0),
        Orientation::AnchorGround | Orientation::Ground => (22.0, 22.0, 25.0, 21.0),
        Orientation::Vertical => (52.0, 45.0 + value_width, 46.0, 46.0),
        _ if is_generic_component(item.component) => (
            component_width / 2.0 + 8.0,
            component_width / 2.0 + 8.0,
            component_height / 2.0 + 8.0,
            component_height / 2.0 + 8.0,
        ),
        _ => (48.0, 48.0, 48.0, 50.0),
    };
    Hitbox {
        left: item.center.x - left_extent - clearance,
        right: item.center.x + right_extent + clearance,
        top: item.center.y - top_extent - clearance,
        bottom: item.center.y + bottom_extent + clearance,
    }
}

pub(super) fn hitboxes_overlap(a: Hitbox, b: Hitbox) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

pub(super) fn prefer_power_on_upper_gate_input(
    placed: &mut [Placed<'_>],
    roots: &BTreeMap<u64, u64>,
) {
    let power_roots = placed
        .iter()
        .filter(|item| {
            matches!(
                item.orientation,
                Orientation::Supply | Orientation::AnchorSupply
            )
        })
        .filter_map(|item| item.component.ports.get("positive"))
        .map(|node| roots.get(node).copied().unwrap_or(*node))
        .collect::<BTreeSet<_>>();

    for item in placed.iter_mut().filter(|item| {
        matches!(
            item.component.kind.as_str(),
            "and-gate" | "or-gate" | "nand-gate" | "nor-gate"
        )
    }) {
        let (Some(a), Some(b)) = (
            item.component.ports.get("a").copied(),
            item.component.ports.get("b").copied(),
        ) else {
            continue;
        };
        let a_root = roots.get(&a).copied().unwrap_or(a);
        let b_root = roots.get(&b).copied().unwrap_or(b);
        if !power_roots.contains(&a_root) && power_roots.contains(&b_root) {
            let (Some(a_point), Some(b_point)) =
                (item.ports.get(&a).copied(), item.ports.get(&b).copied())
            else {
                continue;
            };
            item.ports.insert(a, b_point);
            item.ports.insert(b, a_point);
        }
    }
    for item in placed
        .iter_mut()
        .filter(|item| item.component.kind == "d-flip-flop")
    {
        let (Some(data), Some(clock)) = (
            item.component.ports.get("d").copied(),
            item.component.ports.get("clock").copied(),
        ) else {
            continue;
        };
        let clock_root = roots.get(&clock).copied().unwrap_or(clock);
        if power_roots.contains(&clock_root) {
            let (Some(data_point), Some(clock_point)) = (
                item.ports.get(&data).copied(),
                item.ports.get(&clock).copied(),
            ) else {
                continue;
            };
            item.ports.insert(data, clock_point);
            item.ports.insert(clock, data_point);
        }
    }
}

pub(super) fn align_major_signal_spine(placed: &mut [Placed<'_>], roots: &BTreeMap<u64, u64>) {
    let mut anchors = placed
        .iter()
        .enumerate()
        .filter(|(_, item)| item.component.ports.len() >= 3)
        .map(|(index, item)| (index, item.center.x))
        .collect::<Vec<_>>();
    anchors.sort_by(|a, b| a.1.total_cmp(&b.1));
    for pair in anchors.windows(2) {
        let previous = pair[0].0;
        let next = pair[1].0;
        if placed[previous].section != placed[next].section {
            continue;
        }
        let shared = placed[previous]
            .component
            .ports
            .values()
            .map(|node| roots.get(node).copied().unwrap_or(*node))
            .find(|root| {
                placed[next]
                    .component
                    .ports
                    .values()
                    .any(|node| roots.get(node).copied().unwrap_or(*node) == *root)
            });
        let Some(root) = shared else {
            continue;
        };
        let output = placed[previous].component.ports.values().find_map(|node| {
            (roots.get(node).copied().unwrap_or(*node) == root)
                .then(|| placed[previous].ports.get(node).copied())
                .flatten()
                .filter(|point| point.x > placed[previous].center.x)
        });
        let input = placed[next].component.ports.values().find_map(|node| {
            (roots.get(node).copied().unwrap_or(*node) == root)
                .then(|| placed[next].ports.get(node).copied())
                .flatten()
                .filter(|point| point.x < placed[next].center.x)
        });
        if let (Some(output), Some(input)) = (output, input) {
            let shift = output.y - input.y;
            placed[next].center.y += shift;
            for port in placed[next].ports.values_mut() {
                port.y += shift;
            }
        }
    }
}

pub(super) fn compact_inline_branches(placed: &mut [Placed<'_>], roots: &BTreeMap<u64, u64>) {
    let feeders = placed
        .iter()
        .filter(|item| {
            item.orientation == Orientation::Horizontal && item.component.ports.len() == 2
        })
        .flat_map(|item| {
            item.component.ports.values().filter_map(|node| {
                item.ports.get(node).copied().map(|point| {
                    (
                        item.section.clone(),
                        roots.get(node).copied().unwrap_or(*node),
                        point,
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    for (section, root, feeder) in feeders {
        for item in placed
            .iter_mut()
            .filter(|item| item.orientation == Orientation::Vertical && item.section == section)
        {
            let Some((node, port)) = item.component.ports.values().find_map(|node| {
                (roots.get(node).copied().unwrap_or(*node) == root)
                    .then(|| item.ports.get(node).copied().map(|point| (*node, point)))
                    .flatten()
            }) else {
                continue;
            };
            let offset = port.y - item.center.y;
            let target_y = feeder.y + if offset > 0.0 { 80.0 } else { 120.0 };
            item.center.y = target_y - offset;
            item.ports = port_points(item.component, item.center, item.orientation);
            let _ = node;
        }
    }
}
