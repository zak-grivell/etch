use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fmt::Write;

use circuit_ir::{CircuitDesign, Component};
use render_utils::escape_xml;

mod drawing;
mod placement;
mod routing;
mod topology;

use drawing::*;
use placement::*;
use routing::*;
use topology::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy)]
struct Hitbox {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

struct Placed<'a> {
    component: &'a Component,
    center: Point,
    section: String,
    ports: BTreeMap<u64, Point>,
    orientation: Orientation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Horizontal,
    Vertical,
    Supply,
    AnchorSupply,
    AnchorGround,
    Ground,
}

const SYMBOL_WIDTH: f64 = 80.0;
const SYMBOL_HEIGHT: f64 = 60.0;
const WIRE_LENGTH: f64 = 40.0;
const COLUMN_PITCH: f64 = SYMBOL_WIDTH + WIRE_LENGTH;
const ANCHOR_COLUMN_PITCH: f64 = 220.0;
const WATERFALL_PITCH: f64 = SYMBOL_WIDTH + WIRE_LENGTH;
const ANCHOR_ROW_PITCH: f64 = SYMBOL_HEIGHT + WIRE_LENGTH;

/// Renderer-independent component placement produced by Etch's schematic
/// topology engine. Coordinates are SVG user units; exporters can apply a
/// single uniform scale without changing relative placement.
pub struct SchematicLayout {
    pub components: Vec<ComponentPlacement>,
    pub width: f64,
    pub height: f64,
}

pub struct ComponentPlacement {
    pub x: f64,
    pub y: f64,
    /// Clockwise schematic rotation in degrees.  Multi-pin symbols retain
    /// their library orientation; two-pin parts follow the direction chosen
    /// by the topology pass.
    pub rotation: i32,
}

struct PreparedLayout<'a> {
    sections: Vec<String>,
    roots: BTreeMap<u64, u64>,
    placed: Vec<Placed<'a>>,
    width: f64,
    height: f64,
}

pub fn layout(circuit: &CircuitDesign) -> SchematicLayout {
    let prepared = prepare_layout(circuit);
    let components = circuit
        .components
        .iter()
        .map(|component| {
            let placed = prepared
                .placed
                .iter()
                .find(|placed| std::ptr::eq(placed.component, component))
                .expect("every circuit component must be placed");
            ComponentPlacement {
                x: placed.center.x,
                y: placed.center.y,
                rotation: match placed.orientation {
                    Orientation::Horizontal if component.ports.len() == 2 => 90,
                    _ => 0,
                },
            }
        })
        .collect();
    SchematicLayout {
        components,
        width: prepared.width,
        height: prepared.height,
    }
}

fn prepare_layout(circuit: &CircuitDesign) -> PreparedLayout<'_> {
    let sections = section_names(circuit);
    let roots = circuit.electrical_roots();
    let mut placed: Vec<Placed<'_>> = Vec::new();
    let mut y_offset = 70.0;

    for section in &sections {
        let mut components = circuit
            .components
            .iter()
            .filter(|component| component.section.as_deref().unwrap_or("Circuit") == section)
            .collect::<Vec<_>>();
        // Ground is a terminal decoration, not part of the forward signal flow.
        // Lay it out after active components so it sits below the net it anchors
        // instead of consuming a column and pushing that component downward.
        components.sort_by_key(|component| component.kind == "ground");
        let layout_components = components.clone();
        let layers = graph_layers(&components, &roots);
        let mut rows = BTreeMap::<usize, usize>::new();
        let mut anchor_column = 0usize;
        for (component, layer) in components.into_iter().zip(layers) {
            let row = rows.entry(layer).or_default();
            let orientation = orientation(component, &layout_components, &roots);
            let anchor_x = if orientation == Orientation::Horizontal && component.ports.len() >= 3 {
                let x = 430.0 + anchor_column as f64 * ANCHOR_COLUMN_PITCH;
                anchor_column += 1;
                Some(x)
            } else {
                None
            };
            let mut center = match orientation {
                Orientation::Vertical | Orientation::Supply => Point {
                    x: 110.0 + *row as f64 * COLUMN_PITCH,
                    y: y_offset + 70.0 + layer as f64 * WATERFALL_PITCH,
                },
                Orientation::Horizontal | Orientation::AnchorSupply | Orientation::AnchorGround => {
                    Point {
                        x: anchor_x.unwrap_or(110.0 + layer as f64 * COLUMN_PITCH),
                        y: y_offset + 70.0 + *row as f64 * ANCHOR_ROW_PITCH,
                    }
                }
                Orientation::Ground => Point {
                    x: 110.0,
                    y: y_offset + 70.0 + layer as f64 * WATERFALL_PITCH,
                },
            };
            if orientation == Orientation::Ground
                && let Some(anchor) = placed.iter().rev().find(|item| {
                    item.section == *section
                        && item.component.ports.values().any(|node| {
                            let root = roots.get(node).copied().unwrap_or(*node);
                            component
                                .ports
                                .values()
                                .any(|other| roots.get(other).copied().unwrap_or(*other) == root)
                        })
                })
            {
                let ground_terminal = svg_port(STYLE_AND_SYMBOLS, "symbol-ground", "node")
                    .map(|(_, y)| y.abs())
                    .unwrap_or(SYMBOL_HEIGHT / 2.0);
                center.x = anchor.center.x;
                center.y = anchor.center.y + SYMBOL_WIDTH / 2.0 + WIRE_LENGTH + ground_terminal;
            }
            *row += 1;
            placed.push(Placed {
                component,
                center,
                section: section.clone(),
                ports: port_points(component, center, orientation),
                orientation,
            });
        }
        let section_start = placed
            .iter()
            .position(|item| item.section == *section)
            .unwrap_or(placed.len());
        compact_topology(&mut placed[section_start..], &roots, y_offset + 105.0);
        align_anchor_feeders(&mut placed, section, &roots);
        if let Some(min_y) = placed[section_start..]
            .iter()
            .map(|item| item.center.y)
            .reduce(f64::min)
        {
            let shift = (y_offset + 70.0 - min_y).max(0.0);
            for item in &mut placed[section_start..] {
                item.center.y += shift;
                for port in item.ports.values_mut() {
                    port.y += shift;
                }
            }
        }
        let section_items = placed.iter().filter(|item| item.section == *section);
        let section_bottom = section_items
            .clone()
            .map(|item| item.center.y)
            .reduce(f64::max)
            .unwrap_or(y_offset + 70.0);
        y_offset = section_bottom + 100.0;
    }
    prefer_power_on_upper_gate_input(&mut placed, &roots);
    // Port ordering is semantic (for example, a powered clock belongs above
    // the data input). Re-anchor rail feeders after that ordering changes so
    // their short local drops follow the final port locations.
    for section in &sections {
        align_anchor_feeders(&mut placed, section, &roots);
    }
    align_major_signal_spine(&mut placed, &roots);
    compact_inline_branches(&mut placed, &roots);
    resolve_component_overlaps(circuit, &mut placed, &roots);
    pack_sections(&sections, &mut placed);

    let min_x = placed
        .iter()
        .map(|item| component_hitbox(item, 0.0).left)
        .reduce(f64::min)
        .unwrap_or(20.0);
    let max_x = placed
        .iter()
        .map(|item| component_hitbox(item, 0.0).right)
        .reduce(f64::max)
        .unwrap_or(220.0);
    let side_margin = 50.0;
    let right_margin = circuit
        .net_labels
        .keys()
        .map(|label| label.chars().count() as f64 * 7.2 + 42.0)
        .reduce(f64::max)
        .unwrap_or(side_margin)
        .max(side_margin);
    let x_shift = side_margin - min_x;
    for item in &mut placed {
        item.center.x += x_shift;
        for port in item.ports.values_mut() {
            port.x += x_shift;
        }
    }
    let width = (max_x - min_x + side_margin + right_margin).max(240.0);
    let height = placed
        .iter()
        .map(|item| component_hitbox(item, 0.0).bottom + 70.0)
        .reduce(f64::max)
        .unwrap_or(y_offset)
        .max(220.0);
    PreparedLayout {
        sections,
        roots,
        placed,
        width,
        height,
    }
}

pub fn render(circuit: &CircuitDesign) -> String {
    let PreparedLayout {
        sections,
        roots,
        placed,
        width,
        height,
    } = prepare_layout(circuit);
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    )
    .unwrap();
    svg.push_str(STYLE_AND_SYMBOLS);
    write_custom_symbols(&mut svg, &placed);
    draw_sections(&mut svg, &sections, &placed, width);
    draw_wires(&mut svg, circuit, &placed, &roots);
    draw_components(&mut svg, circuit, &placed, &roots);
    svg.push_str("</svg>\n");
    svg
}

fn pack_sections(sections: &[String], placed: &mut [Placed<'_>]) {
    const TOP: f64 = 70.0;
    const GAP: f64 = 110.0;
    let mut cursor = TOP;
    for section in sections {
        let indices = placed
            .iter()
            .enumerate()
            .filter(|(_, item)| &item.section == section)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if indices.is_empty() {
            continue;
        }
        let top = indices
            .iter()
            .map(|index| component_hitbox(&placed[*index], 0.0).top)
            .reduce(f64::min)
            .unwrap();
        let shift = cursor - top;
        for index in &indices {
            placed[*index].center.y += shift;
            for port in placed[*index].ports.values_mut() {
                port.y += shift;
            }
        }
        cursor = indices
            .iter()
            .map(|index| component_hitbox(&placed[*index], 0.0).bottom)
            .reduce(f64::max)
            .unwrap()
            + GAP;
    }
}
