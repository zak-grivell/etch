use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitDesign, Component, NodeId, PcbConfig};

impl CircuitDesign {
    pub fn add_node(&mut self, node: NodeId) {
        self.nodes.insert(node);
        self.connections.entry(node).or_default();
    }

    pub fn connect(&mut self, a: NodeId, b: NodeId) {
        self.connections.entry(a).or_default().insert(b);
        self.connections.entry(b).or_default().insert(a);
    }

    pub fn add_component(&mut self, component: Component) {
        self.components.push(component);
    }

    pub fn add_section(&mut self, name: String) {
        if !self.sections.contains(&name) {
            self.sections.push(name);
        }
    }

    pub fn add_net_label(&mut self, label: String, node: NodeId) -> Option<NodeId> {
        let nodes = self.net_labels.entry(label).or_default();
        let existing = nodes.first().copied();
        if !nodes.contains(&node) {
            nodes.push(node);
        }
        existing
    }

    pub fn configure_pcb(&mut self, config: PcbConfig) {
        self.pcb = Some(config);
    }

    pub fn electrical_roots(&self) -> BTreeMap<NodeId, NodeId> {
        let mut roots = BTreeMap::new();
        let mut visited = BTreeSet::new();
        for node in self.nodes.iter().copied() {
            if !visited.insert(node) {
                continue;
            }
            let mut members = vec![node];
            let mut stack = vec![node];
            while let Some(current) = stack.pop() {
                for next in self.connections.get(&current).into_iter().flatten() {
                    if visited.insert(*next) {
                        members.push(*next);
                        stack.push(*next);
                    }
                }
            }
            let root = members.iter().copied().min().unwrap_or(node);
            for member in members {
                roots.insert(member, root);
            }
        }
        roots
    }

    pub fn net_names(&self) -> BTreeMap<NodeId, String> {
        let roots = self.electrical_roots();
        let mut names = BTreeMap::new();
        for (label, nodes) in &self.net_labels {
            for node in nodes {
                names.insert(roots.get(node).copied().unwrap_or(*node), label.clone());
            }
        }
        for root in roots.values() {
            names.entry(*root).or_insert_with(|| format!("Net-{root}"));
        }
        names
    }
}
