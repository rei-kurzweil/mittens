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
    /// Use this component's transform -Z even beneath a desktop camera.
    pub parent_forward: bool,
    /// Emit only the nearest geometric hit, ignoring UI interaction priority.
    pub nearest_hit_only: bool,
    pub mode: RayCastMode,

    /// Minimum ray distance in world units. Hits closer than this are ignored.
    ///
    /// The distance is measured from the ray origin emitted by `RayCastSystem`.
    pub min_distance: f32,

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
            parent_forward: false,
            nearest_hit_only: false,
            mode,
            min_distance: 0.0,
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

    /// Set the lower bound of this raycaster's accepted hit interval.
    pub fn with_min_distance(mut self, min_distance: f32) -> Self {
        assert!(
            Self::valid_distance_interval(min_distance, self.max_distance),
            "raycast distance interval requires finite non-negative bounds with min_distance <= max_distance"
        );
        self.min_distance = min_distance;
        self
    }

    pub fn with_max_distance(mut self, max_distance: f32) -> Self {
        assert!(
            Self::valid_distance_interval(self.min_distance, max_distance),
            "raycast distance interval requires finite non-negative bounds with min_distance <= max_distance"
        );
        self.max_distance = max_distance;
        self
    }

    pub fn valid_distance_interval(min_distance: f32, max_distance: f32) -> bool {
        min_distance.is_finite()
            && max_distance.is_finite()
            && min_distance >= 0.0
            && min_distance <= max_distance
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
                component_id: component,
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
                component_id: component,
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
        let expression = ce_call("Raycast", ctor, vec![])
            .with_call("min_distance", vec![num(self.min_distance as f64)])
            .with_call("max_distance", vec![num(self.max_distance as f64)]);
        let expression = if self.parent_forward {
            expression.with_call("parent_forward", vec![b(true)])
        } else {
            expression
        };
        if self.nearest_hit_only {
            expression.with_call("nearest_hit_only", vec![b(true)])
        } else {
            expression
        }
    }
}
