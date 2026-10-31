use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Marks a transform's renderable descendants as XR pointer climbing surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClimbableComponent {
    pub enabled: bool,
}

impl ClimbableComponent {
    pub fn new() -> Self {
        Self::on()
    }
    pub fn on() -> Self {
        Self { enabled: true }
    }
    pub fn off() -> Self {
        Self { enabled: false }
    }
}
impl Default for ClimbableComponent {
    fn default() -> Self {
        Self::new()
    }
}
impl Component for ClimbableComponent {
    fn name(&self) -> &'static str {
        "climbable"
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
                component_id: component,
            },
        );
    }
    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        if self.enabled {
            ce("Climbable")
        } else {
            ce_call("Climbable", "off", vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{CommandQueue, RxWorld, World};
    use crate::scripting::{runner::RuntimeSpecSession, unparser::unparse_component};

    #[test]
    fn on_and_off_round_trip_through_mms() {
        for marker in [ClimbableComponent::on(), ClimbableComponent::off()] {
            let text = unparse_component(&marker.to_mms_ast(&World::default()));
            let mut world = World::default();
            let mut rx = RxWorld::default();
            let mut queue = CommandQueue::new();
            RuntimeSpecSession::start_at_path(
                &text,
                "examples/climbable-test.mms",
                &mut world,
                &mut rx,
                None,
                &mut queue,
            )
            .unwrap();
            let restored = world
                .all_components()
                .find_map(|id| world.get_component_by_id_as::<ClimbableComponent>(id))
                .unwrap();
            assert_eq!(*restored, marker);
        }
    }
}
