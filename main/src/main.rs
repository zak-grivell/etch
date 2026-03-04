mod component;
mod net;
mod resistor;
mod supply;

use net::Node;
use resistor::Resistor;
use supply::VoltageSource;

use crate::component::Edge;

// add BFS but try to follow the direction of current
// brain how to deal with impossible
// add capacitors and inductors

pub fn main() {
    let supply_net = Node::new();
    let ground_net = Node::new();
    let mid1 = Node::new();
    let mid2 = Node::new();
    let mid3 = Node::new();

    let power = Edge::new(VoltageSource::new(
        10.0,
        supply_net.clone(),
        ground_net.clone(),
    ));

    let resistor_one = Edge::new(Resistor::new(1.0, supply_net.clone(), mid1.clone()));
    let resistor_two = Edge::new(Resistor::new(12.0, mid1.clone(), ground_net.clone()));
    let resistor_three = Edge::new(Resistor::new(4.0, mid1.clone(), ground_net.clone()));
    let resistor_four = Edge::new(Resistor::new(1.0, mid1.clone(), mid2.clone()));
    let resistor_five = Edge::new(Resistor::new(10.0, mid2.clone(), ground_net.clone()));
    let resistor_six = Edge::new(Resistor::new(2.0, mid2.clone(), mid3.clone()));
    let resistor_seven = Edge::new(Resistor::new(8.0, mid3.clone(), ground_net.clone()));

    for _ in 0..100 {
        supply_net.predict_voltage();
        mid1.predict_voltage();
        mid2.predict_voltage();
        mid3.predict_voltage();
        ground_net.predict_voltage();

        supply_net.predict_currents();
        mid1.predict_currents();
        mid2.predict_currents();
        mid3.predict_currents();
        ground_net.predict_currents();
    }

    println!("P1: {}", power);
    println!("R1: {}", resistor_one);
    println!("R2: {}", resistor_two);
    println!("R3: {}", resistor_three);
    println!("R4: {}", resistor_four);
    println!("R5: {}", resistor_five);
    println!("R6: {}", resistor_six);
    println!("R7: {}", resistor_seven);

    // println!("Power Node: {}", supply_net);
    // println!("Ref Node: {}", vref_net);
    // println!("Ground Node: {}", ground_net);
}
