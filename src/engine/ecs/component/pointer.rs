use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// User-facing pointer component.
///
/// Attach this under the pose-driving part of the topology (for example a desktop camera rig
/// transform, an XR camera, or a controller-driven transform). At init time the engine spawns
/// and owns a child `RayCastComponent`, so authoring only needs to describe the pointer itself.
#[derive(Debug, Clone, Copy)]
pub struct PointerComponent {
    pub enabled: bool,
    /// Override for the clearance between a held object's ray-facing surface and pointer origin.
    pub min_grab_distance: Option<f32>,

    component: Option<ComponentId>,
}

impl PointerComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            min_grab_distance: None,
            component: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            min_grab_distance: None,
            component: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn min_grab_distance(mut self, meters: f32) -> Self {
        assert!(
            meters.is_finite() && meters >= 0.0,
            "minimum grab distance must be finite and non-negative"
        );
        self.min_grab_distance = Some(meters);
        self
    }
}

impl Default for PointerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PointerComponent {
    fn name(&self) -> &'static str {
        "pointer"
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
            crate::engine::ecs::IntentValue::RegisterPointer {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let expression = if self.enabled {
            ce("Pointer")
        } else {
            ce_call("Pointer", "disabled", vec![])
        };
        match self.min_grab_distance {
            Some(distance) => expression.with_call("min_grab_distance", vec![num(distance as f64)]),
            None => expression,
        }
    }
}
