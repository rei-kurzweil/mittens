use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Marks its immediate parent Transform as movable by XR grip-ray gestures.
#[derive(Debug, Default, Clone, Copy)]
pub struct GrabbableComponent;

impl GrabbableComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for GrabbableComponent {
    fn name(&self) -> &'static str {
        "grabbable"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterGrabbable {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        crate::engine::ecs::component::ce_helpers::ce("Grabbable")
    }
}
