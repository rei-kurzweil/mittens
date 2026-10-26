use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Marks a Transform as attachable to a grabbing pointer while held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrabbableComponent {
    pub enabled: bool,
    /// Handle mode: move the owner's parent Transform instead of the owner itself.
    pub move_parent: bool,
    /// XR hands ease the target's grip origin/orientation into the pointer frame.
    /// Camera/desktop pointers retain their ordinary surface-clearance behavior.
    pub to_hand: bool,
    /// Local translation from the solved hand grip frame while in hand mode.
    pub hand_offset: [f32; 3],
}

impl GrabbableComponent {
    pub fn new() -> Self {
        Self::on()
    }

    pub fn on() -> Self {
        Self {
            enabled: true,
            move_parent: false,
            to_hand: false,
            hand_offset: [0.0; 3],
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            move_parent: false,
            to_hand: false,
            hand_offset: [0.0; 3],
        }
    }

    pub fn parent() -> Self {
        Self {
            enabled: true,
            move_parent: true,
            to_hand: false,
            hand_offset: [0.0; 3],
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
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;

        let authored = if !self.enabled {
            ce_call("Grabbable", "off", vec![])
        } else if self.move_parent {
            ce_call("Grabbable", "parent", vec![])
        } else {
            ce("Grabbable")
        };
        if self.to_hand {
            authored.with_call("to_hand", vec![b(true)])
        } else {
            authored
        }
        .with_call(
            "hand_offset",
            vec![array(
                self.hand_offset
                    .into_iter()
                    .map(|v| num(v as f64))
                    .collect(),
            )],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{CommandQueue, RxWorld, World};
    use crate::scripting::runner::RuntimeSpecSession;
    use crate::scripting::unparser::unparse_component;

    #[test]
    fn hand_flag_round_trips_with_enabled_disabled_and_parent_modes() {
        for mut authored in [
            GrabbableComponent::on(),
            GrabbableComponent::off(),
            GrabbableComponent::parent(),
        ] {
            authored.to_hand = true;
            let text = unparse_component(&authored.to_mms_ast(&World::default()));
            assert!(text.contains("to_hand(true)"), "{text}");
            let mut world = World::default();
            let mut rx = RxWorld::default();
            let mut queue = CommandQueue::new();
            let (_session, _) = RuntimeSpecSession::start_at_path(
                &text,
                "examples/grabbable-to-hand-test.mms",
                &mut world,
                &mut rx,
                None,
                &mut queue,
            )
            .unwrap();
            let restored = world
                .all_components()
                .find_map(|id| world.get_component_by_id_as::<GrabbableComponent>(id))
                .unwrap();
            assert_eq!(*restored, authored);
        }
        assert!(!GrabbableComponent::new().to_hand);
    }
}
