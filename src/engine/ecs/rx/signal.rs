use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::component::Action;
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone)]
pub enum ActionSignal {
    /// Intent: execute an action.
    Action(Action),
}

#[derive(Debug, Clone)]
pub enum EventSignal {
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
    ///
    /// `delta` is the vector from `a` to `b` in world space: `pos(b) - pos(a)`.
    CollisionStarted {
        a: ComponentId,
        b: ComponentId,
        delta: [f32; 3],
    },

    /// Fact: two collision objects stopped overlapping this tick.
    ///
    /// `delta` is the last known vector from `a` to `b` in world space: `pos(b) - pos(a)`.
    CollisionEnded {
        a: ComponentId,
        b: ComponentId,
        delta: [f32; 3],
    },

    /// Fact: a drag gesture started.
    DragStart {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],

        /// Optional screen-space cursor/pointer position in pixels.
        ///
        /// Present for screen-space pointers (mouse/touch). Absent for non-screen pointers.
        screen_pos_px: Option<(f32, f32)>,
    },

    /// Fact: a drag gesture moved this tick.
    ///
    /// `delta_world` is the world-space movement since the last DragMove for this gesture.
    DragMove {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],
        delta_world: [f32; 3],

        /// Optional screen-space cursor/pointer position in pixels.
        screen_pos_px: Option<(f32, f32)>,

        /// Optional pixel delta since the previous DragMove for this drag.
        ///
        /// Present for screen-space pointers (mouse/touch) when previous screen position is
        /// known. Absent for non-screen pointers.
        screen_delta_px: Option<(f32, f32)>,
    },

    /// Fact: a drag gesture ended.
    DragEnd {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: Option<[f32; 3]>,
    },
}

#[derive(Debug, Clone)]
pub enum SignalValue {
    Action(ActionSignal),
    Event(EventSignal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    Any,
    Action,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
    DragStart,
    DragMove,
    DragEnd,
}

impl SignalValue {
    pub fn kind(&self) -> SignalKind {
        match self {
            SignalValue::Action(ActionSignal::Action(_)) => SignalKind::Action,
            SignalValue::Event(EventSignal::ParentChanged { .. }) => SignalKind::ParentChanged,
            SignalValue::Event(EventSignal::RayIntersected { .. }) => SignalKind::RayIntersected,
            SignalValue::Event(EventSignal::CollisionStarted { .. }) => {
                SignalKind::CollisionStarted
            }
            SignalValue::Event(EventSignal::CollisionEnded { .. }) => SignalKind::CollisionEnded,
            SignalValue::Event(EventSignal::DragStart { .. }) => SignalKind::DragStart,
            SignalValue::Event(EventSignal::DragMove { .. }) => SignalKind::DragMove,
            SignalValue::Event(EventSignal::DragEnd { .. }) => SignalKind::DragEnd,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub scope: ComponentId,
    pub value: SignalValue,
}

impl Signal {
    pub fn kind(&self) -> SignalKind {
        self.value.kind()
    }
}

impl From<ActionSignal> for SignalValue {
    fn from(v: ActionSignal) -> Self {
        SignalValue::Action(v)
    }
}

impl From<EventSignal> for SignalValue {
    fn from(v: EventSignal) -> Self {
        SignalValue::Event(v)
    }
}

impl From<Action> for ActionSignal {
    fn from(v: Action) -> Self {
        ActionSignal::Action(v)
    }
}

impl From<Action> for SignalValue {
    fn from(v: Action) -> Self {
        SignalValue::Action(ActionSignal::Action(v))
    }
}

pub type SignalHandler = fn(&mut World, &mut CommandQueue, &Signal);
