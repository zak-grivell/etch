use circuit_ir::sexpr::SExpr;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Pin {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

pub(crate) struct LibrarySymbol {
    pub(crate) definition: String,
    pub(crate) pins: BTreeMap<String, Pin>,
}

pub(crate) fn load(library_id: &str) -> Result<Option<LibrarySymbol>, String> {
    let Some((library, symbol)) = library_id.split_once(':') else {
        return Err(format!("invalid KiCad symbol library ID `{library_id}`"));
    };
    let Some(path) = symbol_directories()
        .into_iter()
        .map(|directory| directory.join(format!("{library}.kicad_sym")))
        .find(|path| path.is_file())
    else {
        return Ok(None);
    };
    let source = fs::read_to_string(&path).map_err(|error| {
        format!(
            "could not read KiCad symbol library {}: {error}",
            path.display()
        )
    })?;
    let Some(mut definition) = resolve_definition(&source, symbol, &mut Vec::new())? else {
        return Err(format!(
            "KiCad symbol `{library_id}` was not found in {}",
            path.display()
        ));
    };
    let original_name = opening_name(&definition).unwrap().to_owned();
    definition = rename_definition(&definition, &original_name, library_id);
    let pins = unit_one_pins(&definition)?;
    if pins.is_empty() {
        return Err(format!("KiCad symbol `{library_id}` has no pins"));
    }
    Ok(Some(LibrarySymbol { definition, pins }))
}

fn unit_one_pins(definition: &str) -> Result<BTreeMap<String, Pin>, String> {
    let parsed = SExpr::parse(definition)?;
    let mut pins = BTreeMap::new();
    for unit in parsed
        .items()
        .iter()
        .filter(|item| item.tag() == Some("symbol"))
    {
        let name = unit.value(1).unwrap_or("");
        let unit_number = name
            .rsplit('_')
            .nth(1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        let style = name
            .rsplit('_')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1);
        if unit_number <= 1 && style <= 1 {
            pins.extend(extract_pins(&unit.to_string()));
        }
    }
    Ok(pins)
}

fn symbol_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(path) = std::env::var_os("KICAD_SYMBOL_DIR") {
        directories.push(path.into());
    }
    directories.extend([
        PathBuf::from("/Applications/KiCad/KiCad.app/Contents/SharedSupport/symbols"),
        PathBuf::from("/usr/share/kicad/symbols"),
        PathBuf::from("/usr/local/share/kicad/symbols"),
    ]);
    if let Some(program_files) = std::env::var_os("PROGRAMFILES") {
        let root = PathBuf::from(program_files).join("KiCad");
        if let Ok(entries) = fs::read_dir(root) {
            directories.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path().join("share/kicad/symbols")),
            );
        }
    }
    directories
}

fn resolve_definition(
    source: &str,
    name: &str,
    resolving: &mut Vec<String>,
) -> Result<Option<String>, String> {
    if resolving.iter().any(|item| item == name) {
        return Err(format!(
            "cyclic KiCad symbol inheritance involving `{name}`"
        ));
    }
    let Some(definition) = find_definition(source, name) else {
        return Ok(None);
    };
    let Some(base) = field_string(&definition, "extends") else {
        return Ok(Some(definition));
    };
    resolving.push(name.to_owned());
    let resolved = resolve_definition(source, &base, resolving)?;
    resolving.pop();
    resolved
        .map(|base_definition| {
            merge_inherited(
                &rename_definition(&base_definition, &base, name),
                &definition,
            )
        })
        .transpose()?
        .map(Some)
        .ok_or_else(|| format!("base KiCad symbol `{base}` for `{name}` was not found"))
}

fn merge_inherited(base: &str, child: &str) -> Result<String, String> {
    let mut base = SExpr::parse(base)?;
    let child = SExpr::parse(child)?;
    for field in child.items().iter().skip(2) {
        if field.tag() == Some("extends") {
            continue;
        }
        let tag = field.tag();
        let name = field.value(1);
        base.items_mut()
            .retain(|old| !(old.tag() == tag && (tag != Some("property") || old.value(1) == name)));
        base.items_mut().push(field.clone());
    }
    Ok(base.to_string())
}

fn find_definition(source: &str, name: &str) -> Option<String> {
    let needle = format!("(symbol \"{name}\"");
    let start = source
        .match_indices(&needle)
        .find_map(|(start, _)| (expression_depth(&source[..start]) == 1).then_some(start))?;
    balanced_expression(source, start).map(str::to_owned)
}

fn expression_depth(source: &str) -> isize {
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    for character in source.chars() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if character == '(' {
            depth += 1;
        } else if character == ')' {
            depth -= 1;
        }
    }
    depth
}

fn balanced_expression(source: &str, start: usize) -> Option<&str> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for (offset, character) in source[start..].char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if character == '(' {
            depth += 1;
        } else if character == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(&source[start..start + offset + 1]);
            }
        }
    }
    None
}

fn opening_name(definition: &str) -> Option<&str> {
    definition.strip_prefix("(symbol \"")?.split('"').next()
}

fn rename_definition(definition: &str, old: &str, new: &str) -> String {
    definition
        .replacen(
            &format!("(symbol \"{old}\""),
            &format!("(symbol \"{new}\""),
            1,
        )
        .replace(
            &format!("\"{old}_"),
            &format!("\"{}_", new.rsplit(':').next().unwrap_or(new)),
        )
}

fn field_string(source: &str, field: &str) -> Option<String> {
    let start = source.find(&format!("({field} \""))? + field.len() + 3;
    Some(source[start..].split('"').next()?.to_owned())
}

fn extract_pins(definition: &str) -> BTreeMap<String, Pin> {
    let mut pins = BTreeMap::new();
    let mut offset = 0;
    while let Some(relative) = definition[offset..].find("(pin ") {
        let start = offset + relative;
        let Some(pin) = balanced_expression(definition, start) else {
            break;
        };
        if let (Some(at), Some(number)) = (field_values(pin, "at"), field_string(pin, "number"))
            && at.len() >= 2
            && let (Ok(x), Ok(y)) = (at[0].parse(), at[1].parse())
        {
            pins.entry(number).or_insert(Pin { x, y });
        }
        offset = start + pin.len();
    }
    pins
}

fn field_values<'a>(source: &'a str, field: &str) -> Option<Vec<&'a str>> {
    let start = source.find(&format!("({field} "))? + field.len() + 2;
    let end = source[start..].find(')')? + start;
    Some(source[start..end].split_whitespace().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_renames_symbol_definitions() {
        let source = r#"(kicad_symbol_lib
          (symbol "Base"
            (symbol "Base_1_1"
              (pin passive line (at -5.08 2.54 0) (number "1"))))
          (symbol "Part" (extends "Base")))"#;
        let definition = resolve_definition(source, "Part", &mut Vec::new())
            .unwrap()
            .unwrap();
        let definition = rename_definition(&definition, "Part", "Library:Part");
        assert!(definition.starts_with("(symbol \"Library:Part\""));
        assert!(definition.contains("(symbol \"Part_1_1\""));
        let pins = extract_pins(&definition);
        assert_eq!(pins["1"].x, -5.08);
        assert_eq!(pins["1"].y, 2.54);
    }
}

#[cfg(test)]
mod followup_tests {
    use super::*;
    #[test]
    fn derived_symbol_properties_override_the_base() {
        let source = r#"(kicad_symbol_lib
            (symbol "Base" (property "Reference" "U") (property "Value" "Base") (property "Footprint" "Old:Part")
                (symbol "Base_1_1" (pin passive line (at 1 2 0) (number "1"))))
            (symbol "Child" (extends "Base") (property "Value" "Child") (property "Footprint" "New:Part")))"#;
        let definition = resolve_definition(source, "Child", &mut Vec::new())
            .unwrap()
            .unwrap();
        assert!(definition.contains(r#"(property "Reference" "U")"#));
        assert!(definition.contains(r#"(property "Footprint" "New:Part")"#));
        assert!(!definition.contains("Old:Part"));
        assert_eq!(unit_one_pins(&definition).unwrap().len(), 1);
    }
    #[test]
    fn later_units_and_alternate_styles_are_not_exported_as_unit_one() {
        let source = r#"(symbol "Dual" (symbol "Dual_0_1" (pin passive line (at 0 0 0) (number "8")))
          (symbol "Dual_1_1" (pin passive line (at 1 0 0) (number "1")))
          (symbol "Dual_2_1" (pin passive line (at 2 0 0) (number "2")))
          (symbol "Dual_1_2" (pin passive line (at 3 0 0) (number "3"))))"#;
        let pins = unit_one_pins(source).unwrap();
        assert_eq!(pins.keys().cloned().collect::<Vec<_>>(), vec!["1", "8"]);
    }
}
