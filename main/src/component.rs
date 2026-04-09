use std::{
    cell::{Ref, RefCell, RefMut},
    fmt,
    hash::Hash,
    rc::Rc,
};

use uuid::Uuid;

use crate::net::Node;

pub trait Component {
    fn step(&mut self);

    fn current(&self) -> f64;
    fn voltage(&self) -> f64;

    fn connections(&self) -> Vec<Node>;
    fn id(&self) -> Uuid;
}

#[derive(Clone)]
pub struct Edge {
    component: Rc<RefCell<dyn Component>>,
}

impl Edge {
    pub fn new(component: impl Component + 'static) -> Self {
        let edge = Edge {
            component: Rc::new(RefCell::new(component)),
        };

        edge.borrow().connections().iter().for_each(|c| {
            c.connect(&edge);
        });

        edge
    }

    pub fn borrow(&self) -> Ref<'_, dyn Component + 'static> {
        self.component.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, dyn Component + 'static> {
        self.component.borrow_mut()
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        return self.borrow().id() == other.borrow().id();
    }
}

impl Eq for Edge {}

impl Hash for Edge {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.borrow().id().hash(state);
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "V: {:.3}, I: {:.3}",
            self.borrow().voltage(),
            self.borrow().current()
        )
    }
}
