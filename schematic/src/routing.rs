pub(super) fn draw_orthogonal_net(
    svg: &mut String,
    terminals: &[Point],
    placed: &[Placed<'_>],
    circuit: &CircuitDesign,
    roots: &BTreeMap<u64, u64>,
    occupied: &[(Point, Point)],
) -> Result<Vec<(Point, Point)>, ()> {
    let mut drawing = String::new();
    let mut segments = Vec::new();
    if terminals.len() < 2 {
        return Ok(segments);
    }
    let root_index = terminals
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| {
            let major_penalty = if terminal_owner(**candidate, placed)
                .is_some_and(|index| placed[index].component.ports.len() >= 3)
            {
                0_u64
            } else {
                1_000_000
            };
            let distance = terminals
                .iter()
                .map(|other| ((candidate.x - other.x).abs() + (candidate.y - other.y).abs()) as u64)
                .sum::<u64>();
            major_penalty + distance
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    let root = terminals[root_index];
    let mut remaining = terminals
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != root_index)
        .map(|(_, point)| *point)
        .collect::<Vec<_>>();
    remaining.sort_by_key(|point| {
        let major = terminal_owner(*point, placed)
            .is_some_and(|index| placed[index].component.ports.len() >= 3);
        (
            !major,
            ((point.x - root.x).abs() + (point.y - root.y).abs()) as u64,
        )
    });
    let mut tree_points = vec![root];
    for terminal in remaining {
        let path = if tree_points.len() == 1 {
            route_between(root, terminal, placed, circuit, roots)
        } else {
            tree_points
                .iter()
                .filter_map(|tree_point| {
                    let path = route_between(terminal, *tree_point, placed, circuit, roots);
                    (!path.is_empty()).then(|| (route_cost(&path), path))
                })
                .min_by_key(|(cost, _)| *cost)
                .map(|(_, path)| path)
                .unwrap_or_default()
        };
        if path.len() < 2 {
            return Err(());
        }
        for pair in path.windows(2) {
            if occupied
                .iter()
                .any(|(a, b)| segments_intersect(pair[0], pair[1], *a, *b))
            {
                return Err(());
            }
            for (index, item) in placed.iter().enumerate() {
                if Some(index) != terminal_owner(path[0], placed)
                    && Some(index) != terminal_owner(*path.last().unwrap(), placed)
                    && !segment_clear(pair[0], pair[1], &[routing_hitbox(item, 0.0)])
                {
                    return Err(());
                }
            }
            segments.push((pair[0], pair[1]));
        }
        let mut data = format!("M {} {}", path[0].x, path[0].y);
        for pair in path.windows(2) {
            let next = pair[1];
            if (pair[0].x - next.x).abs() < f64::EPSILON {
                write!(data, " V {}", next.y).unwrap();
            } else {
                write!(data, " H {}", next.x).unwrap();
            }
        }
        writeln!(drawing, r#"<path class="wire" d="{data}"/>"#).unwrap();
        let existing = tree_points.clone();
        for point in &path {
            if !tree_points
                .iter()
                .any(|candidate| same_point(*candidate, *point))
            {
                tree_points.push(*point);
            }
        }
        for pair in path.windows(2) {
            let midpoint = Point {
                x: (pair[0].x + pair[1].x) / 2.0,
                y: (pair[0].y + pair[1].y) / 2.0,
            };
            if !tree_points
                .iter()
                .any(|candidate| same_point(*candidate, midpoint))
            {
                tree_points.push(midpoint);
            }
        }
        if existing.len() > 1 {
            let junction = *path.last().unwrap();
            writeln!(
                drawing,
                r#"<circle class="junction" cx="{}" cy="{}" r="2.5"/>"#,
                junction.x, junction.y
            )
            .unwrap();
        }
    }
    svg.push_str(&drawing);
    Ok(segments)
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let epsilon = 0.01;
    a.x.min(b.x) <= c.x.max(d.x) + epsilon
        && c.x.min(d.x) <= a.x.max(b.x) + epsilon
        && a.y.min(b.y) <= c.y.max(d.y) + epsilon
        && c.y.min(d.y) <= a.y.max(b.y) + epsilon
}

pub(super) fn route_cost(path: &[Point]) -> u64 {
    let distance = path
        .windows(2)
        .map(|pair| ((pair[0].x - pair[1].x).abs() + (pair[0].y - pair[1].y).abs()) as u64)
        .sum::<u64>();
    distance + path.len().saturating_sub(2) as u64 * 30
}

pub(super) fn route_between(
    start: Point,
    goal: Point,
    placed: &[Placed<'_>],
    circuit: &CircuitDesign,
    roots: &BTreeMap<u64, u64>,
) -> Vec<Point> {
    let start_escape = escape_point(start, placed);
    let goal_escape = escape_point(goal, placed);
    let start_owner = terminal_owner(start, placed);
    let goal_owner = terminal_owner(goal, placed);
    let obstacles = placed
        .iter()
        .enumerate()
        .filter(|(index, item)| {
            Some(*index) != start_owner
                && Some(*index) != goal_owner
                && component_is_visible(circuit, item, roots)
        })
        .map(|(_, item)| routing_hitbox(item, 8.0))
        .collect::<Vec<_>>();

    // Prefer the compact dogleg that continues in the terminal's facing
    // direction. Besides producing fewer arbitrary steps, this lets ordered
    // rail lanes nest cleanly instead of sharing a vertical segment merely
    // because both Manhattan alternatives have the same length.
    for corner in [
        Point {
            x: goal_escape.x,
            y: start_escape.y,
        },
        Point {
            x: start_escape.x,
            y: goal_escape.y,
        },
    ] {
        let path = simplify_path(vec![start, start_escape, corner, goal_escape, goal]);
        if path
            .windows(2)
            .all(|segment| segment_clear(segment[0], segment[1], &obstacles))
        {
            return path;
        }
    }

    let mut xs = vec![start_escape.x, goal_escape.x];
    let mut ys = vec![start_escape.y, goal_escape.y];
    for obstacle in &obstacles {
        xs.extend([obstacle.left - 8.0, obstacle.right + 8.0]);
        ys.extend([obstacle.top - 8.0, obstacle.bottom + 8.0]);
    }
    let outer_left = obstacles
        .iter()
        .map(|obstacle| obstacle.left)
        .chain([start_escape.x, goal_escape.x])
        .reduce(f64::min)
        .unwrap_or(0.0)
        - WIRE_LENGTH;
    let outer_right = obstacles
        .iter()
        .map(|obstacle| obstacle.right)
        .chain([start_escape.x, goal_escape.x])
        .reduce(f64::max)
        .unwrap_or(0.0)
        + WIRE_LENGTH;
    let outer_top = obstacles
        .iter()
        .map(|obstacle| obstacle.top)
        .chain([start_escape.y, goal_escape.y])
        .reduce(f64::min)
        .unwrap_or(0.0)
        - WIRE_LENGTH;
    let outer_bottom = obstacles
        .iter()
        .map(|obstacle| obstacle.bottom)
        .chain([start_escape.y, goal_escape.y])
        .reduce(f64::max)
        .unwrap_or(0.0)
        + WIRE_LENGTH;
    xs.extend([outer_left, outer_right]);
    ys.extend([outer_top, outer_bottom]);
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
    ys.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
    let start_index = grid_index(&xs, &ys, start_escape).unwrap();
    let goal_index = grid_index(&xs, &ys, goal_escape).unwrap();
    let node_count = xs.len() * ys.len();
    let mut distance = vec![i64::MAX; node_count];
    let mut previous = vec![None; node_count];
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
        let x_index = node % xs.len();
        let y_index = node / xs.len();
        let mut neighbors = Vec::with_capacity(4);
        if x_index > 0 {
            neighbors.push(node - 1);
        }
        if x_index + 1 < xs.len() {
            neighbors.push(node + 1);
        }
        if y_index > 0 {
            neighbors.push(node - xs.len());
        }
        if y_index + 1 < ys.len() {
            neighbors.push(node + xs.len());
        }
        let from = grid_point(&xs, &ys, node);
        for neighbor in neighbors {
            let to = grid_point(&xs, &ys, neighbor);
            if !segment_clear(from, to, &obstacles) {
                continue;
            }
            let step = ((from.x - to.x).abs() + (from.y - to.y).abs()).round() as i64;
            let next_cost = cost + step;
            if next_cost < distance[neighbor] {
                distance[neighbor] = next_cost;
                previous[neighbor] = Some(node);
                queue.push((Reverse(next_cost), neighbor));
            }
        }
    }

    if distance[goal_index] == i64::MAX {
        return Vec::new();
    }
    let mut nodes = vec![goal_index];
    while *nodes.last().unwrap() != start_index {
        nodes.push(previous[*nodes.last().unwrap()].unwrap());
    }
    nodes.reverse();
    let mut path = vec![start];
    path.extend(nodes.into_iter().map(|node| grid_point(&xs, &ys, node)));
    path.push(goal);
    let path = simplify_path(path);
    debug_assert!(
        path.len() < 4
            || path[1..path.len() - 1]
                .windows(2)
                .all(|segment| segment_clear(segment[0], segment[1], &obstacles))
    );
    path
}

pub(super) fn terminal_owner(point: Point, placed: &[Placed<'_>]) -> Option<usize> {
    placed
        .iter()
        .position(|item| item.ports.values().any(|port| same_point(*port, point)))
}

pub(super) fn escape_point(point: Point, placed: &[Placed<'_>]) -> Point {
    let Some(owner) = terminal_owner(point, placed).map(|index| &placed[index]) else {
        return point;
    };
    let dx = point.x - owner.center.x;
    let dy = point.y - owner.center.y;
    let hitbox = routing_hitbox(owner, 8.0);
    if dx.abs() >= dy.abs() {
        Point {
            x: if dx < 0.0 {
                hitbox.left - 8.0
            } else {
                hitbox.right + 8.0
            },
            y: point.y,
        }
    } else {
        Point {
            x: point.x,
            y: if dy < 0.0 {
                hitbox.top - 8.0
            } else {
                hitbox.bottom + 8.0
            },
        }
    }
}

pub(super) fn grid_index(xs: &[f64], ys: &[f64], point: Point) -> Option<usize> {
    let x = xs
        .iter()
        .position(|value| (*value - point.x).abs() < f64::EPSILON)?;
    let y = ys
        .iter()
        .position(|value| (*value - point.y).abs() < f64::EPSILON)?;
    Some(y * xs.len() + x)
}

pub(super) fn grid_point(xs: &[f64], ys: &[f64], node: usize) -> Point {
    Point {
        x: xs[node % xs.len()],
        y: ys[node / xs.len()],
    }
}

pub(super) fn segment_clear(a: Point, b: Point, obstacles: &[Hitbox]) -> bool {
    obstacles.iter().all(|obstacle| {
        if (a.y - b.y).abs() < f64::EPSILON {
            !(a.y > obstacle.top
                && a.y < obstacle.bottom
                && a.x.max(b.x) > obstacle.left
                && a.x.min(b.x) < obstacle.right)
        } else {
            !(a.x > obstacle.left
                && a.x < obstacle.right
                && a.y.max(b.y) > obstacle.top
                && a.y.min(b.y) < obstacle.bottom)
        }
    })
}

pub(super) fn simplify_path(path: Vec<Point>) -> Vec<Point> {
    let mut simplified: Vec<Point> = Vec::new();
    for point in path {
        if simplified.len() >= 2 {
            let a = simplified[simplified.len() - 2];
            let b = simplified[simplified.len() - 1];
            if ((a.x - b.x).abs() < f64::EPSILON && (b.x - point.x).abs() < f64::EPSILON)
                || ((a.y - b.y).abs() < f64::EPSILON && (b.y - point.y).abs() < f64::EPSILON)
            {
                simplified.pop();
            }
        }
        if simplified
            .last()
            .is_none_or(|last| !same_point(*last, point))
        {
            simplified.push(point);
        }
    }
    simplified
}

pub(super) fn same_point(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < f64::EPSILON && (a.y - b.y).abs() < f64::EPSILON
}
use super::*;
