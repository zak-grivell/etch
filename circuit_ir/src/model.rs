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
