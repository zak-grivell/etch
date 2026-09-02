use std::fmt::Write;

use super::TraceSeries;

const WIDTH: f64 = 900.0;
const HEIGHT: f64 = 480.0;
const LEFT: f64 = 78.0;
const RIGHT: f64 = 24.0;
const TOP: f64 = 54.0;
const BOTTOM: f64 = 58.0;
const COLORS: [&str; 8] = [
    "#ffd166", "#06d6a0", "#4cc9f0", "#f72585", "#b8f2e6", "#ff9f1c", "#c77dff", "#ef476f",
];

pub(super) fn render(title: &str, traces: &[TraceSeries]) -> String {
    let samples = traces
        .iter()
        .flat_map(|trace| trace.samples.iter().copied())
        .collect::<Vec<_>>();
    let (mut min_x, mut max_x) = bounds(samples.iter().map(|(time, _)| *time));
    let (mut min_y, mut max_y) = bounds(samples.iter().map(|(_, value)| *value));
    normalize_range(&mut min_x, &mut max_x);
    normalize_range(&mut min_y, &mut max_y);
    let y_padding = (max_y - min_y) * 0.08;
    min_y -= y_padding;
    max_y += y_padding;

    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let x = |value: f64| LEFT + (value - min_x) / (max_x - min_x) * plot_width;
    let y = |value: f64| TOP + (max_y - value) / (max_y - min_y) * plot_height;

    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">"#
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
        r#"<rect class="background" width="{WIDTH}" height="{HEIGHT}"/><rect class="plot" x="{LEFT}" y="{TOP}" width="{plot_width}" height="{plot_height}"/><text class="title" x="{LEFT}" y="30">{}</text>"#,
        escape(title)
    )
    .unwrap();

    for tick in 0..=5 {
        let ratio = tick as f64 / 5.0;
        let px = LEFT + ratio * plot_width;
        let py = TOP + ratio * plot_height;
        let x_value = min_x + ratio * (max_x - min_x);
        let y_value = max_y - ratio * (max_y - min_y);
        writeln!(
            svg,
            r#"<path class="grid" d="M {px} {TOP} V {}"/><text class="time" x="{px}" y="{}">{}</text>"#,
            TOP + plot_height,
            TOP + plot_height + 22.0,
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
        r#"<path class="axis" d="M {LEFT} {TOP} V {} H {}"/><text class="time" x="{}" y="{}">time (s)</text>"#,
        TOP + plot_height,
        LEFT + plot_width,
        LEFT + plot_width / 2.0,
        HEIGHT - 12.0
    )
    .unwrap();

    for (index, trace) in traces.iter().enumerate() {
        let color = COLORS[index % COLORS.len()];
        let mut path = String::new();
        for (sample, (time, value)) in trace.samples.iter().enumerate() {
            if time.is_finite() && value.is_finite() {
                write!(
                    path,
                    "{} {} {} ",
                    if sample == 0 { "M" } else { "L" },
                    x(*time),
                    y(*value)
                )
                .unwrap();
            }
        }
        writeln!(
            svg,
            r#"<path class="trace" stroke="{color}" d="{path}"/><g><path stroke="{color}" stroke-width="3" d="M {} {} h 22"/><text x="{}" y="{}">{}</text></g>"#,
            LEFT + 15.0 + index as f64 * 145.0,
            TOP + 17.0,
            LEFT + 42.0 + index as f64 * 145.0,
            TOP + 21.0,
            escape(&trace.name)
        )
        .unwrap();
    }
    svg.push_str("</svg>\n");
    svg
}

fn bounds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let values = values.filter(|value| value.is_finite()).collect::<Vec<_>>();
    (
        values.iter().copied().reduce(f64::min).unwrap_or(0.0),
        values.iter().copied().reduce(f64::max).unwrap_or(1.0),
    )
}

fn normalize_range(min: &mut f64, max: &mut f64) {
    if (*max - *min).abs() < f64::EPSILON {
        let padding = min.abs().max(1.0) * 0.1;
        *min -= padding;
        *max += padding;
    }
}

fn format_number(value: f64) -> String {
    if value != 0.0 && (value.abs() < 0.001 || value.abs() >= 10000.0) {
        format!("{value:.2e}")
    } else {
        format!("{value:.3}")
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
