use std::{cell::RefCell, fmt, hash::Hash, rc::Rc};

use uuid::Uuid;

use crate::net::Node;

#[derive(Debug)]
pub enum SimResult {
    Exact(f64),
    Predict(f64),
    None,
}

pub trait Component {
    fn predict_voltage(&self, node: &Node) -> SimResult;
    fn predict_current(&self, node: &Node) -> SimResult;

    fn current(&self) -> f64;
    fn voltage(&self) -> f64;

    fn connections(&self) -> Vec<Node>;

    fn id(&self) -> Uuid;
}

#[derive(Clone)]
pub struct Edge(Rc<RefCell<dyn Component>>);

impl Edge {
    pub fn predict_voltage(&self, net: &Node) -> SimResult {
        self.0.borrow().predict_voltage(net)
    }

    pub fn predict_current(&self, net: &Node) -> SimResult {
        self.0.borrow().predict_current(net)
    }

    pub fn current(&self) -> f64 {
        self.0.borrow().current()
    }

    pub fn voltage(&self) -> f64 {
        self.0.borrow().voltage()
    }

    pub fn new(component: impl Component + 'static) -> Self {
        let connections: Vec<_> = component.connections().iter().cloned().collect();

        let edge = Edge(Rc::new(RefCell::new(component)));

        connections.iter().for_each(|c| {
            c.connect(&edge);
        });

        return edge;
    }

    pub fn id(&self) -> Uuid {
        return self.0.borrow().id();
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        return self.id() == other.id();
    }
}

impl Eq for Edge {}

impl Hash for Edge {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V: {:.3}, I: {:.3}", self.voltage(), self.current())
    }
}
