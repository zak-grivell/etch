use std::collections::{BTreeMap, BTreeSet};

pub type NodeId = u64;

#[derive(Clone, Debug, Default)]
pub struct CircuitDesign {
    pub nodes: BTreeSet<NodeId>,
    pub connections: BTreeMap<NodeId, BTreeSet<NodeId>>,
    pub components: Vec<Component>,
    pub net_labels: BTreeMap<String, Vec<NodeId>>,
    pub sections: Vec<String>,
    pub pcb: Option<PcbConfig>,
}

#[derive(Clone, Debug)]
pub struct Component {
    pub kind: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub ports: BTreeMap<String, NodeId>,
    pub section: Option<String>,
    pub svg: Option<String>,
    pub kicad: Option<KicadLink>,
}

#[derive(Clone, Debug)]
pub struct KicadLink {
    pub symbol: String,
    pub footprint: String,
    pub pins: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct PcbConfig {
    pub width: f64,
    pub height: f64,
    pub layers: usize,
    pub min_trace_width: f64,
    pub clearance: f64,
}

impl PcbConfig {
    pub fn validate(&self) -> Result<(), String> {
        if ![
            self.width,
            self.height,
            self.min_trace_width,
            self.clearance,
        ]
        .iter()
        .all(|v| v.is_finite())
            || self.width <= 0.0
            || self.height <= 0.0
            || self.min_trace_width <= 0.0
            || self.clearance < 0.0
            || !(1..=16).contains(&self.layers)
        {
            return Err("PCB dimensions and trace width must be positive and finite, clearance nonnegative and finite, and layers between 1 and 16".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraceSeries {
    pub name: String,
    pub samples: Vec<(f64, f64)>,
}
