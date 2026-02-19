use crate::engine::ecs::component::Action;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::ecs::CommandQueue;

#[derive(Debug, Clone)]
pub enum Signal {
    /// Intent: execute an action.
    Action(Action),

    /// Fact: topology changed.
    ParentChanged {
        child: ComponentId,
        old_parent: Option<ComponentId>,
        new_parent: Option<ComponentId>,
    },

    /// Fact: a raycast intersected a renderable.
    RayIntersected {
        raycaster: ComponentId,
        renderable: ComponentId,
        t: f32,
        origin: [f32; 3],
        dir: [f32; 3],
    },

    /// Fact: two collision objects began overlapping this tick.
    CollisionStarted {
        a: ComponentId,
        b: ComponentId,
    },

    /// Fact: two collision objects stopped overlapping this tick.
    CollisionEnded {
        a: ComponentId,
        b: ComponentId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    Any,
    Action,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
}

impl Signal {
    pub fn kind(&self) -> SignalKind {
        match self {
            Signal::Action(_) => SignalKind::Action,
            Signal::ParentChanged { .. } => SignalKind::ParentChanged,
            Signal::RayIntersected { .. } => SignalKind::RayIntersected,
            Signal::CollisionStarted { .. } => SignalKind::CollisionStarted,
            Signal::CollisionEnded { .. } => SignalKind::CollisionEnded,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignalEnvelope {
    pub scope: ComponentId,
    pub signal: Signal,
}

pub type SignalHandler = fn(&mut World, &mut CommandQueue, &SignalEnvelope);
