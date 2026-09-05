use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitDesign, Component, NodeId, PcbConfig};

impl CircuitDesign {
    pub fn add_node(&mut self, node: NodeId) {
        self.nodes.insert(node);
        self.connections.entry(node).or_default();
    }

    pub fn connect(&mut self, a: NodeId, b: NodeId) {
        self.add_node(a);
        self.add_node(b);
        self.connections.entry(a).or_default().insert(b);
        self.connections.entry(b).or_default().insert(a);
    }

    pub fn add_component(&mut self, component: Component) {
        for node in component.ports.values() {
            self.add_node(*node);
        }
        if let Some(section) = &component.section {
            self.add_section(section.clone());
        }
        self.components.push(component);
    }

    pub fn add_section(&mut self, name: String) {
        if !self.sections.contains(&name) {
            self.sections.push(name);
        }
    }

    pub fn add_net_label(&mut self, label: String, node: NodeId) -> Option<NodeId> {
        self.add_node(node);
        let nodes = self.net_labels.entry(label).or_default();
        let existing = nodes.first().copied();
        if !nodes.contains(&node) {
            nodes.push(node);
        }
        if let Some(previous) = existing {
            self.connect(previous, node);
        }
        existing
    }

    pub fn section_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self
            .components
            .iter()
            .any(|component| component.section.is_none())
        {
            names.push("Circuit".into());
        }
        for name in self.sections.iter().chain(
            self.components
                .iter()
                .filter_map(|component| component.section.as_ref()),
        ) {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        if names.is_empty() {
            names.push("Circuit".into());
        }
        names
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registers_connected_and_labelled_nodes() {
        let mut design = CircuitDesign::default();
        design.connect(1, 2);
        design.add_net_label("signal".into(), 2);
        design.add_net_label("signal".into(), 3);
        assert_eq!(
            design.electrical_roots(),
            BTreeMap::from([(1, 1), (2, 1), (3, 1)])
        );
    }
    #[test]
    fn derives_unique_sections_from_components() {
        let mut design = CircuitDesign::default();
        for section in [None, Some("Circuit".into()), Some("Other".into())] {
            design.add_component(Component {
                kind: "test".into(),
                label: None,
                value: None,
                ports: BTreeMap::from([("pin".into(), 7)]),
                section,
                svg: None,
                kicad: None,
            });
        }
        assert_eq!(design.section_names(), vec!["Circuit", "Other"]);
        assert!(design.nodes.contains(&7));
    }
}
