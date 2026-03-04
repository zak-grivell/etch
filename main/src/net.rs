use crate::component::{Edge, SimResult};
use core::fmt;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use uuid::Uuid;

struct Net {
    id: Uuid,

    voltage: f64,
    connections: Vec<Edge>,
    current: HashMap<Uuid, f64>,
}

#[derive(Clone)]
pub struct Node(Rc<RefCell<Net>>);

impl Node {
    pub fn new() -> Node {
        Node(Rc::new(RefCell::new(Net {
            voltage: 0.0,
            id: Uuid::new_v4(),
            current: HashMap::new(),
            connections: Vec::new(),
        })))
    }

    pub fn connections(&self) -> Vec<Edge> {
        self.0.borrow().connections.clone()
    }

    pub fn connect(&self, component: &Edge) {
        self.0.borrow_mut().connections.push(component.clone());
        self.0.borrow_mut().current.insert(component.id(), 0.0);
    }

    pub fn voltage(&self) -> f64 {
        self.0.borrow().voltage
    }

    pub fn current_to(&self, id: &Uuid) -> f64 {
        *self.0.borrow().current.get(id).unwrap()
    }

    pub fn predict_voltage(&self) {
        let sim_results = self
            .connections()
            .iter()
            .map(|v| v.predict_voltage(self))
            .collect::<Vec<_>>();

        let exacts = sim_results
            .iter()
            .filter_map(|r| match r {
                SimResult::Exact(f) => Some(*f),
                _ => None,
            })
            .collect::<Vec<_>>();

        if exacts.len() == 0 {
            let predicts = sim_results
                .iter()
                .filter_map(|r| match r {
                    SimResult::Predict(f) => Some(*f),
                    _ => None,
                })
                .fold((0.0, 0), |(sum, count), v| (sum + v, count + 1));

            self.0.borrow_mut().voltage = predicts.0 / predicts.1 as f64;
        } else if exacts.len() == 1 {
            self.0.borrow_mut().voltage = exacts[0];
        } else {
            if (exacts.iter().sum::<f64>() / exacts.len() as f64 - exacts[0]).abs() > 1e-6 {
                panic!(
                    "Conflicting voltage predictions for node {}: {:?}",
                    self.0.borrow().id,
                    exacts
                );
            }

            return;
        }
    }

    pub fn predict_currents(&self) {
        let s = self
            .0
            .borrow()
            .connections
            .iter()
            .map(|component| {
                (
                    component.id(),
                    match component.predict_current(self) {
                        SimResult::Exact(f) => SimResult::Exact(f),
                        SimResult::Predict(f) => SimResult::Predict(f),
                        SimResult::None => SimResult::Predict(
                            *self.0.borrow().current.get(&component.id()).unwrap_or(&0.0),
                        ),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let total = s
            .values()
            .filter_map(|x| match x {
                SimResult::Predict(f) => Some(*f),
                SimResult::Exact(f) => Some(*f),
                _ => None,
            })
            .sum::<f64>();

        let adjustable = s
            .values()
            .filter_map(|x| match x {
                SimResult::Predict(_) => Some(()),
                SimResult::None => Some(()),
                _ => None,
            })
            .count();

        if adjustable == 0 {
            if total.abs() > 1e-6 {
                panic!(
                    "No adjustable currents but total current is not zero for node {}",
                    self.0.borrow().id
                );
            }

            return;
        }

        let delta = -total / adjustable as f64;

        self.0.borrow_mut().current = s
            .iter()
            .map(|(k, r)| match r {
                SimResult::Exact(v) => (*k, *v),
                SimResult::Predict(v) => (*k, v + delta),
                SimResult::None => (*k, delta),
            })
            .collect();
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        return self.0.borrow().id == other.0.borrow().id;
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "V: {}, I: {:?}",
            self.voltage(),
            self.0
                .borrow()
                .current
                .values()
                .cloned()
                .collect::<Vec<_>>()
        )
    }
}
