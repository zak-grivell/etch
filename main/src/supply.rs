use crate::{component::SimResult, net::Node};
use uuid::Uuid;

use crate::component::Component;

pub struct VoltageSource {
    voltage: f64,
    out: Node,
    gnd: Node,
    id: Uuid,
}

impl VoltageSource {
    pub fn new(voltage: f64, out: Node, gnd: Node) -> VoltageSource {
        VoltageSource {
            voltage,
            out: out.clone(),
            gnd: gnd.clone(),
            id: Uuid::new_v4(),
        }
    }
}

impl Component for VoltageSource {
    fn predict_voltage(&self, net: &Node) -> SimResult {
        if *net == self.out {
            SimResult::Exact(self.voltage)
        } else {
            SimResult::Exact(0.0)
        }
    }

    fn predict_current(&self, _net: &Node) -> SimResult {
        SimResult::None
    }

    fn current(&self) -> f64 {
        return (self.out.current_to(&self.id).abs() + self.gnd.current_to(&self.id).abs()) / 2.0;
    }

    fn voltage(&self) -> f64 {
        return self.out.voltage() - self.gnd.voltage();
    }

    fn connections(&self) -> Vec<Node> {
        vec![self.out.clone(), self.gnd.clone()]
    }

    fn id(&self) -> Uuid {
        self.id
    }
}
