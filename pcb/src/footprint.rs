use super::Point;
use circuit_ir::{Component, sexpr::SExpr};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Clone)]
pub struct Footprint {
    pub definition: SExpr,
    pub pads: Vec<Pad>,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone)]
pub struct Pad {
    pub number: String,
    pub node: Option<u64>,
    pub point: Point,
    pub width: f64,
    pub height: f64,
    pub through: bool,
}
impl Footprint {
    pub fn load(component: &Component) -> Result<Self, String> {
        let link = component.kicad.as_ref().ok_or_else(|| {
            format!(
                "component `{}` requires a KiCad footprint link",
                component.kind
            )
        })?;
        let (library, name) = link
            .footprint
            .split_once(':')
            .filter(|(a, b)| {
                !a.is_empty()
                    && !b.is_empty()
                    && !a.contains(['/', '\\'])
                    && !b.contains(['/', '\\'])
            })
            .ok_or_else(|| format!("invalid footprint identifier `{}`", link.footprint))?;
        let mut directories = Vec::new();
        if let Some(path) = std::env::var_os("KICAD_FOOTPRINT_DIR") {
            directories.push(PathBuf::from(path));
        }
        directories.extend([
            PathBuf::from("/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints"),
            PathBuf::from("/usr/share/kicad/footprints"),
            PathBuf::from("/usr/local/share/kicad/footprints"),
        ]);
        if let Some(root) = std::env::var_os("PROGRAMFILES")
            && let Ok(entries) = fs::read_dir(PathBuf::from(root).join("KiCad"))
        {
            directories.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path().join("share/kicad/footprints")),
            );
        }
        let path = directories.into_iter().map(|dir| dir.join(format!("{library}.pretty/{name}.kicad_mod"))).find(|path| path.is_file()).ok_or_else(|| format!("footprint `{}` not found; set KICAD_FOOTPRINT_DIR to your KiCad footprint directory",link.footprint))?;
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_source(component, &source)
    }
    pub fn from_source(component: &Component, source: &str) -> Result<Self, String> {
        let definition = SExpr::parse(source)?;
        if definition.tag() != Some("footprint") {
            return Err("expected a KiCad footprint".into());
        }
        let link = component.kicad.as_ref().ok_or("missing footprint link")?;
        let mut numbers = BTreeMap::new();
        for (port, node) in &component.ports {
            let number = link
                .pins
                .get(port)
                .ok_or_else(|| format!("missing pin map for `{port}`"))?;
            if numbers.insert(number.clone(), *node).is_some() {
                return Err(format!("duplicate pin mapping `{number}`"));
            }
        }
        let mut pads = Vec::new();
        let (mut width, mut height) = (0.0_f64, 0.0_f64);
        for item in definition.items() {
            if item.tag() == Some("pad") {
                let number = item.value(1).ok_or("pad without number")?.to_owned();
                let at = item.field("at").ok_or("pad without position")?;
                let size = item.field("size").ok_or("pad without size")?;
                let (x, y) = (at.number(1)?, -at.number(2)?);
                let (w, h) = (size.number(1)?, size.number(2)?);
                let angle = at.value(3).map_or(Ok(0.0), |_| at.number(3))?.to_radians();
                let (w, h) = (
                    w * angle.cos().abs() + h * angle.sin().abs(),
                    w * angle.sin().abs() + h * angle.cos().abs(),
                );
                width = width.max(2.0 * x.abs() + w);
                height = height.max(2.0 * y.abs() + h);
                pads.push(Pad {
                    node: numbers.get(&number).copied(),
                    number,
                    point: Point { x, y },
                    width: w,
                    height: h,
                    through: item.value(2) == Some("thru_hole"),
                });
            } else if matches!(item.tag(), Some("fp_line" | "fp_rect")) {
                for field in ["start", "end"] {
                    if let Some(point) = item.field(field) {
                        width = width.max(2.0 * point.number(1)?.abs());
                        height = height.max(2.0 * point.number(2)?.abs());
                    }
                }
            }
        }
        for number in numbers.keys() {
            if !pads.iter().any(|pad| &pad.number == number) {
                return Err(format!(
                    "footprint `{}` has no mapped pad `{number}`",
                    link.footprint
                ));
            }
        }
        if pads.is_empty() {
            return Err("footprint has no pads".into());
        }
        Ok(Self {
            definition,
            pads,
            width: width.max(0.1),
            height: height.max(0.1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_ir::KicadLink;
    #[test]
    fn keeps_distinct_physical_pads_on_the_same_node() {
        let component = Component {
            kind: "part".into(),
            label: None,
            value: None,
            section: None,
            svg: None,
            ports: BTreeMap::from([("a".into(), 7), ("b".into(), 7)]),
            kicad: Some(KicadLink {
                symbol: "Device:R".into(),
                footprint: "Test:Part".into(),
                pins: BTreeMap::from([("a".into(), "1".into()), ("b".into(), "2".into())]),
            }),
        };
        let source = r#"(footprint "Part" (pad "1" smd rect (at -1 2) (size 1 2) (layers "F.Cu")) (pad "2" thru_hole circle (at 1 -2) (size 2 2) (drill 1) (layers "*.Cu")))"#;
        let footprint = Footprint::from_source(&component, source).unwrap();
        let pads = super::super::component_pads(&footprint, Point { x: 10.0, y: 20.0 }, 90);
        assert_eq!(pads.len(), 2);
        assert_eq!(pads[0].node, Some(7));
        assert_eq!(pads[1].node, Some(7));
        assert_eq!((pads[0].point.x, pads[0].point.y), (12.0, 19.0));
        assert_eq!((pads[1].point.x, pads[1].point.y), (8.0, 21.0));
        assert!(!pads[0].through);
        assert!(pads[1].through);
        let missing = source.replace("pad \"2\"", "pad \"3\"");
        assert!(Footprint::from_source(&component, &missing).is_err());
    }
}
