use crate::{component::Component, net::Node};
use uuid::Uuid;

pub struct Resistor {
    resistance: f64,
    a: Node,
    b: Node,
    id: Uuid,
}

impl Resistor {
    pub fn new(resistance: f64, a: Node, b: Node) -> Resistor {
        Resistor {
            resistance,
            a: a.clone(),
            b: b.clone(),
            id: Uuid::new_v4(),
        }
    }
}

impl Component for Resistor {
    fn step(&mut self) {
        let v_a = self.a.voltage();
        let v_b = self.b.voltage();

        let g = 1.0 / self.resistance;

        self.a.push(v_b * g, g);
        self.b.push(v_a * g, g);
    }

    fn current(&self) -> f64 {
        (self.a.voltage() - self.b.voltage()) / self.resistance
    }

    fn voltage(&self) -> f64 {
        self.a.voltage() - self.b.voltage()
    }

    fn connections(&self) -> Vec<Node> {
        vec![self.a.clone(), self.b.clone()]
    }

    fn id(&self) -> Uuid {
        self.id
    }
}
