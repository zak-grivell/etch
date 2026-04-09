use crate::component::Edge;
use core::fmt;
use std::{
    cell::{Ref, RefCell, RefMut},
    hash::Hash,
    rc::Rc,
};
use uuid::Uuid;

struct Net {
    id: Uuid,

    voltage: f64,
    connections: Vec<Edge>,

    acc_current: f64,
    acc_conductance: f64,
    is_fixed: bool,
}

#[derive(Clone)]
pub struct Node(Rc<RefCell<Net>>);

impl Node {
    pub fn new() -> Node {
        Node(Rc::new(RefCell::new(Net {
            voltage: 0.0,
            id: Uuid::new_v4(),
            connections: Vec::new(),
            acc_current: 0.0,
            acc_conductance: 0.0,
            is_fixed: false,
        })))
    }

    fn borrow(&self) -> Ref<'_, Net> {
        self.0.borrow()
    }

    fn borrow_mut(&self) -> RefMut<'_, Net> {
        self.0.borrow_mut()
    }

    pub fn connect(&self, component: &Edge) {
        self.borrow_mut().connections.push(component.clone());
    }

    pub fn voltage(&self) -> f64 {
        self.borrow().voltage
    }

    pub fn set_fixed(&self, voltage: f64) {
        let mut n = self.borrow_mut();
        n.voltage = voltage;
        n.is_fixed = true;
    }

    pub fn push(&self, current: f64, conductance: f64) {
        let mut n = self.borrow_mut();
        n.acc_current += current;
        n.acc_conductance += conductance;
    }

    pub fn clear(&self) {
        let mut n = self.borrow_mut();
        n.acc_current = 0.0;
        n.acc_conductance = 0.0;
    }

    pub fn update_voltage(&self) {
        let mut n = self.borrow_mut();
        if n.is_fixed {
            return;
        }

        if n.acc_conductance > 0.0 {
            n.voltage = n.acc_current / n.acc_conductance;
        }
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        return self.0.borrow().id == other.0.borrow().id;
    }
}

impl Eq for Node {}

impl Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.borrow().id.hash(state);
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V: {:.3}", self.voltage())
    }
}
