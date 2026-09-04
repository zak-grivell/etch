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
    pub(crate) height: f64,
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
    let pins = extract_pins(&definition);
    if pins.is_empty() {
        return Err(format!("KiCad symbol `{library_id}` has no pins"));
    }
    let min_y = pins.values().map(|pin| pin.y).fold(f64::INFINITY, f64::min);
    let max_y = pins
        .values()
        .map(|pin| pin.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(Some(LibrarySymbol {
        definition,
        pins,
        height: (max_y - min_y).abs() + 10.16,
    }))
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
        .map(|definition| rename_definition(&definition, &base, name))
        .map(Some)
        .ok_or_else(|| format!("base KiCad symbol `{base}` for `{name}` was not found"))
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
