use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RayCastMode {
    Continuous,
    EventDriven,
}

/// Ray casting request/behavior.
///
/// Semantics:
/// - Attach this anywhere (commonly under a camera rig transform).
/// - The RayCastSystem resolves the actual ray source from surrounding topology.
/// - `EventDriven` means the raycaster casts only when explicitly requested, except for
///   desktop cursor-through-camera pointers which currently auto-cast from desktop mouse input.
#[derive(Debug, Clone, Copy)]
pub struct RayCastComponent {
    pub mode: RayCastMode,

    /// Max ray distance in world units.
    pub max_distance: f32,

    /// Incremented by `IntentValue::RequestRaycast` to request a cast on this frame.
    ///
    /// This is intentionally not serialized; it is a transient runtime signal.
    pub cast_requests: u32,

    component: Option<ComponentId>,
}

impl RayCastComponent {
    pub fn new(mode: RayCastMode) -> Self {
        Self {
            mode,
            max_distance: 200.0,
            cast_requests: 0,
            component: None,
        }
    }

    pub fn continuous() -> Self {
        Self::new(RayCastMode::Continuous)
    }

    pub fn event_driven() -> Self {
        Self::new(RayCastMode::EventDriven)
    }

    pub fn with_max_distance(mut self, max_distance: f32) -> Self {
        self.max_distance = max_distance;
        self
    }
}

impl Default for RayCastComponent {
    fn default() -> Self {
        Self::event_driven()
    }
}

impl Component for RayCastComponent {
    fn name(&self) -> &'static str {
        "raycast"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterRaycast {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveRaycast {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.mode {
            RayCastMode::Continuous => "continuous",
            RayCastMode::EventDriven => "event_driven",
        };
        ce_call("Raycast", ctor, vec![])
            .with_call("max_distance", vec![num(self.max_distance as f64)])
    }
}
