use crate::{
    component::{Component, SimResult},
    net::Node,
};
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
    fn predict_voltage(&self, net: &Node) -> SimResult {
        if net == &self.a {
            SimResult::Predict(self.b.voltage() + self.current() * self.resistance)
        } else if net == &self.b {
            SimResult::Predict(self.a.voltage() - self.current() * self.resistance)
        } else {
            SimResult::None
        }
    }

    fn predict_current(&self, net: &Node) -> SimResult {
        if net == &self.a {
            SimResult::Predict((self.a.voltage() - self.b.voltage()) / self.resistance)
        } else if net == &self.b {
            SimResult::Predict((self.b.voltage() - self.a.voltage()) / self.resistance)
        } else {
            SimResult::None
        }
    }

    fn current(&self) -> f64 {
        return (self.a.current_to(&self.id).abs() + self.b.current_to(&self.id).abs()) / 2.0;
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
