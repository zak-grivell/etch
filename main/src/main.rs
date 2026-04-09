mod component;
mod graph;
mod net;
mod resistor;
mod supply;

use net::Node;
use resistor::Resistor;
use supply::VoltageSource;

use crate::{component::Edge, graph::GraphApp};

pub fn main() {
    let mut graph = GraphApp::new();

    let i1 = graph.plot("1");
    let i2 = graph.plot("1");
    let i3 = graph.plot("1");

    let supply_net = Node::new();
    let ground_net = Node::new();
    let mid_net = Node::new();

    ground_net.set_fixed(0.0);

    let power = Edge::new(VoltageSource::new(
        2.0,
        supply_net.clone(),
        ground_net.clone(),
    ));
    let resistor_one = Edge::new(Resistor::new(0.000001, supply_net.clone(), mid_net.clone()));
    let resistor_two = Edge::new(Resistor::new(1.0, mid_net.clone(), ground_net.clone()));

    let components = vec![power.clone(), resistor_one.clone(), resistor_two.clone()];
    let nodes = vec![supply_net.clone(), ground_net.clone(), mid_net.clone()];

    for _ in 0..1000 {
        for node in &nodes {
            node.clear();
        }

        for comp in &components {
            comp.borrow_mut().step();
        }

        for node in &nodes {
            node.update_voltage();
        }

        i1.add(supply_net.voltage());
        i2.add(mid_net.voltage());
        i3.add(ground_net.voltage());
    }

    println!("P1: {}", power);
    println!("R1: {}", resistor_one);
    println!("R2: {}", resistor_two);

    graph.run();
}
