use crate::net::Node;
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
    fn step(&mut self) {
        let v_out = self.out.voltage();
        let v_gnd = self.gnd.voltage();

        // Model as a low-resistance source with high conductance
        let g = 10000.0;

        // self.out.set_fixed(self.voltage);

        // V_out should be V_gnd + voltage
        self.out.push((v_gnd + self.voltage) * g, g);
        // V_gnd should be V_out - voltage
        self.gnd.push((v_out - self.voltage) * g, g);
    }

    fn current(&self) -> f64 {
        let v_out = self.out.voltage();
        let v_gnd = self.gnd.voltage();
        let g = 100.0;
        (v_gnd + self.voltage - v_out) * g
    }

    fn voltage(&self) -> f64 {
        self.out.voltage() - self.gnd.voltage()
    }

    fn connections(&self) -> Vec<Node> {
        vec![self.out.clone(), self.gnd.clone()]
    }

    fn id(&self) -> Uuid {
        self.id
    }
}
