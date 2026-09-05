use render_utils::escape_xml;
use std::fmt::Write;

pub use circuit_ir::TraceSeries;

const WIDTH: f64 = 900.0;
const HEIGHT: f64 = 480.0;
const LEFT: f64 = 78.0;
const RIGHT: f64 = 24.0;
const TOP: f64 = 54.0;
const BOTTOM: f64 = 58.0;
const COLORS: [&str; 8] = [
    "#ffd166", "#06d6a0", "#4cc9f0", "#f72585", "#b8f2e6", "#ff9f1c", "#c77dff", "#ef476f",
];

pub fn render(title: &str, traces: &[TraceSeries]) -> String {
    let x_axis = Axis::new(
        traces
            .iter()
            .flat_map(|trace| trace.samples.iter().map(|p| p.0)),
        false,
    );
    let y_axis = Axis::new(
        traces
            .iter()
            .flat_map(|trace| trace.samples.iter().map(|p| p.1)),
        true,
    );
    let widths: Vec<f64> = traces
        .iter()
        .map(|trace| 48.0 + trace.name.chars().count() as f64 * 7.3)
        .collect();
    let width = widths.iter().copied().fold(WIDTH - LEFT - RIGHT, f64::max) + LEFT + RIGHT;
    let mut legend = Vec::new();
    let mut cursor = LEFT;
    let mut row = 0;
    for entry_width in widths {
        if cursor > LEFT && cursor + entry_width > width - RIGHT {
            row += 1;
            cursor = LEFT;
        }
        legend.push((cursor, TOP + row as f64 * 24.0));
        cursor += entry_width;
    }
    let extra = if traces.is_empty() {
        0.0
    } else {
        (row + 1) as f64 * 24.0
    };
    let top = TOP + extra;
    let height = HEIGHT + extra;
    let plot_width = width - LEFT - RIGHT;
    let plot_height = height - top - BOTTOM;
    let x = |value: f64| LEFT + x_axis.ratio(value) * plot_width;
    let y = |value: f64| top + (1.0 - y_axis.ratio(value)) * plot_height;

    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    )
    .unwrap();
    svg.push_str(
        r#"<style>
            .background{fill:#10151d}.plot{fill:#151c26;stroke:#566174}
            .grid{stroke:#344052;stroke-width:1}.axis{stroke:#8c99ad;stroke-width:1.5}
            text{font-family:ui-monospace,monospace;fill:#c9d2df;font-size:12px}
            .title{font-size:17px;font-weight:600}.tick{text-anchor:end}.time{text-anchor:middle}
            .trace{fill:none;stroke-width:2.2;stroke-linejoin:round;stroke-linecap:round}
        </style>"#,
    );
    writeln!(
        svg,
        r#"<rect class="background" width="{width}" height="{height}"/><rect class="plot" x="{LEFT}" y="{top}" width="{plot_width}" height="{plot_height}"/><text class="title" x="{LEFT}" y="30">{}</text>"#,
        escape_xml(title)
    )
    .unwrap();

    for tick in 0..=5 {
        let ratio = tick as f64 / 5.0;
        let px = LEFT + ratio * plot_width;
        let py = top + ratio * plot_height;
        let x_value = x_axis.value(ratio);
        let y_value = y_axis.value(1.0 - ratio);
        writeln!(
            svg,
            r#"<path class="grid" d="M {px} {top} V {}"/><text class="time" x="{px}" y="{}">{}</text>"#,
            top + plot_height,
            top + plot_height + 22.0,
            format_number(x_value)
        )
        .unwrap();
        writeln!(
            svg,
            r#"<path class="grid" d="M {LEFT} {py} H {}"/><text class="tick" x="{}" y="{}">{}</text>"#,
            LEFT + plot_width,
            LEFT - 10.0,
            py + 4.0,
            format_number(y_value)
        )
        .unwrap();
    }
    writeln!(
        svg,
        r#"<path class="axis" d="M {LEFT} {top} V {} H {}"/><text class="time" x="{}" y="{}">time (s)</text>"#,
        top + plot_height,
        LEFT + plot_width,
        LEFT + plot_width / 2.0,
        height - 12.0
    )
    .unwrap();

    for (index, trace) in traces.iter().enumerate() {
        let color = COLORS[index % COLORS.len()];
        let mut path = String::new();
        let mut has_point = false;
        for (time, value) in &trace.samples {
            if time.is_finite() && value.is_finite() {
                write!(
                    path,
                    "{} {} {} ",
                    if has_point { "L" } else { "M" },
                    x(*time),
                    y(*value)
                )
                .unwrap();
                has_point = true;
            } else {
                has_point = false;
            }
        }
        writeln!(
            svg,
            r#"<path class="trace" stroke="{color}" d="{path}"/><g><path stroke="{color}" stroke-width="3" d="M {} {} h 22"/><text x="{}" y="{}">{}</text></g>"#,
            legend[index].0,
            legend[index].1 - 4.0,
            legend[index].0 + 28.0,
            legend[index].1,
            escape_xml(&trace.name)
        )
        .unwrap();
    }
    svg.push_str("</svg>\n");
    svg
}

struct Axis {
    scale: f64,
    low: f64,
    high: f64,
}
impl Axis {
    fn new(values: impl Iterator<Item = f64>, padding: bool) -> Self {
        let bounds = values
            .filter(|v| v.is_finite())
            .fold(None, |bounds, value| {
                Some(match bounds {
                    None => (value, value),
                    Some((low, high)) => (f64::min(low, value), f64::max(high, value)),
                })
            });
        let (low, high) = bounds.unwrap_or((0.0, 1.0));
        let scale = low.abs().max(high.abs()).max(f64::MIN_POSITIVE);
        let (mut low, mut high) = (low / scale, high / scale);
        let gap = if low == high {
            0.1
        } else if padding {
            (high - low) * 0.08
        } else {
            0.0
        };
        low = (low - gap).max(-1.0);
        high = (high + gap).min(1.0);
        Self { scale, low, high }
    }
    fn ratio(&self, value: f64) -> f64 {
        (value / self.scale - self.low) / (self.high - self.low)
    }
    fn value(&self, ratio: f64) -> f64 {
        (self.low * (1.0 - ratio) + self.high * ratio).clamp(-1.0, 1.0) * self.scale
    }
}

fn format_number(value: f64) -> String {
    if value != 0.0 && (value.abs() < 0.001 || value.abs() >= 10000.0) {
        format!("{value:.2e}")
    } else {
        format!("{value:.3}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_a_trace_at_the_first_finite_sample() {
        let svg = render(
            "non-finite samples",
            &[TraceSeries {
                name: "signal".into(),
                samples: vec![(f64::NAN, 0.0), (1.0, 2.0), (2.0, 3.0)],
            }],
        );

        let trace = svg
            .split("class=\"trace\"")
            .nth(1)
            .and_then(|tail| tail.split("d=\"").nth(1))
            .and_then(|tail| tail.split('"').next())
            .unwrap();
        assert!(trace.starts_with("M "), "trace path was {trace:?}");
        assert!(!trace.contains("NaN"));
    }
}

#[cfg(test)]
#[test]
fn leaves_a_gap_for_missing_samples() {
    let svg = render(
        "gap",
        &[TraceSeries {
            name: "x".into(),
            samples: vec![(0.0, 1.0), (1.0, f64::NAN), (2.0, 3.0)],
        }],
    );
    let trace = svg
        .split("class=\"trace\"")
        .nth(1)
        .unwrap()
        .split("d=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(trace.matches("M ").count(), 2);
    assert!(!trace.contains("L "));
}

#[cfg(test)]
#[test]
fn extreme_values_and_long_legends_remain_inside_a_finite_canvas() {
    let traces: Vec<_> = (0..20)
        .map(|index| TraceSeries {
            name: format!("trace {index}: {}", "long name ".repeat(20)),
            samples: vec![(-f64::MAX, -f64::MAX), (f64::MAX, f64::MAX)],
        })
        .collect();
    let svg = render("extremes", &traces);
    assert!(!svg.contains("NaN"));
    assert!(!svg.contains("inf"));
    assert_eq!(svg.matches("class=\"trace\"").count(), 20);
    let flat = render(
        "constant",
        &[TraceSeries {
            name: "constant".into(),
            samples: vec![(f64::MAX, f64::MAX)],
        }],
    );
    assert!(!flat.contains("NaN"));
    assert!(!flat.contains("inf"));
}
