use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Marks its immediate parent Transform as movable by pointer drag gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrabbableComponent {
    pub enabled: bool,
    /// Handle mode: move the owner's parent Transform instead of the owner itself.
    pub move_parent: bool,
}

impl GrabbableComponent {
    pub fn new() -> Self {
        Self::on()
    }

    pub fn on() -> Self {
        Self {
            enabled: true,
            move_parent: false,
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            move_parent: false,
        }
    }

    pub fn parent() -> Self {
        Self {
            enabled: true,
            move_parent: true,
        }
    }
}

impl Default for GrabbableComponent {
    fn default() -> Self {
        Self::new()
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
        if !self.enabled {
            crate::engine::ecs::component::ce_helpers::ce_call("Grabbable", "off", vec![])
        } else if self.move_parent {
            crate::engine::ecs::component::ce_helpers::ce_call("Grabbable", "parent", vec![])
        } else {
            crate::engine::ecs::component::ce_helpers::ce("Grabbable")
        }
    }
}
