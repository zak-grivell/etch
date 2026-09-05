use circuit_ir::CircuitDesign;

#[derive(Clone, Copy)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}
pub(crate) fn ensure_links(circuit: &CircuitDesign) -> Result<(), String> {
    for component in &circuit.components {
        let link = component.kicad.as_ref().ok_or_else(|| format!(
            "component kind `{}` has no KiCad link; add kicad: {{ symbol, footprint, pins }} to use_symbol", component.kind
        ))?;
        if !link
            .symbol
            .split_once(':')
            .is_some_and(|(library, name)| !library.is_empty() && !name.is_empty())
        {
            return Err(format!("invalid KiCad symbol identifier `{}`", link.symbol));
        }
        let mut numbers = std::collections::BTreeSet::new();
        for port in component.ports.keys() {
            let number = link.pins.get(port).ok_or_else(|| {
                format!(
                    "component `{}` has no KiCad pin mapping for port `{port}`",
                    component.kind
                )
            })?;
            if number.is_empty() || !numbers.insert(number) {
                return Err(format!(
                    "component `{}` has an empty or repeated KiCad pin number",
                    component.kind
                ));
            }
        }
    }
    Ok(())
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

pub(crate) fn references(circuit: &CircuitDesign) -> Result<Vec<String>, String> {
    let mut counts = std::collections::BTreeMap::new();
    let mut prefixes = std::collections::BTreeMap::new();
    circuit
        .components
        .iter()
        .map(|component| {
            let library = component.kicad.as_ref().map_or("", |link| &link.symbol);
            if !prefixes.contains_key(library) {
                let prefix = if let Some(symbol) = crate::symbol_library::load(library)? {
                    let definition = circuit_ir::sexpr::SExpr::parse(&symbol.definition)?;
                    definition
                        .items()
                        .iter()
                        .find(|item| {
                            item.tag() == Some("property") && item.value(1) == Some("Reference")
                        })
                        .and_then(|item| item.value(2))
                        .map(str::to_owned)
                        .unwrap_or_else(|| reference_prefix(library))
                } else {
                    reference_prefix(library)
                };
                prefixes.insert(library.to_owned(), prefix);
            }
            let prefix = &prefixes[library];
            let count = counts.entry(prefix.clone()).or_insert(0);
            *count += 1;
            Ok(format!("{prefix}{count}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_ir::{Component, KicadLink};
    use std::collections::BTreeMap;
    #[test]
    fn exporters_reject_missing_pin_mappings_without_panicking() {
        let mut design = CircuitDesign::default();
        design.add_component(Component {
            kind: "resistor".into(),
            label: None,
            value: None,
            ports: BTreeMap::from([("a".into(), 1)]),
            section: None,
            svg: None,
            kicad: Some(KicadLink {
                symbol: "Device:R".into(),
                footprint: String::new(),
                pins: BTreeMap::new(),
            }),
        });
        assert!(
            crate::schematic(&design)
                .unwrap_err()
                .contains("pin mapping")
        );
        assert!(crate::pcb(&design).unwrap_err().contains("pin mapping"));
    }
}
