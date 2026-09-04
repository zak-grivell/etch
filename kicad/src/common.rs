use circuit_ir::CircuitDesign;

#[derive(Clone, Copy)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}
pub(crate) fn ensure_links(circuit: &CircuitDesign) -> Result<(), String> {
    circuit
        .components
        .iter()
        .find(|component| component.kicad.is_none())
        .map_or(Ok(()), |component| {
            Err(format!(
                "component kind `{}` has no KiCad link; add kicad: {{ symbol, footprint, pins }} to use_symbol",
                component.kind
            ))
        })
}

pub(crate) fn reference_prefix(symbol: &str) -> String {
    let name = symbol.rsplit(':').next().unwrap_or(symbol);
    match name.chars().next().unwrap_or('U').to_ascii_uppercase() {
        'R' => "R",
        'C' => "C",
        'L' => "L",
        'D' => "D",
        'V' => "V",
        'I' => "I",
        'J' => "J",
        _ => "U",
    }
    .into()
}

pub(crate) fn uuid(component: usize, item: usize) -> String {
    format!("00000000-0000-4000-8000-{component:06x}{item:06x}")
}

pub(crate) fn esc(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}
