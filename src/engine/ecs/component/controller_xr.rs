use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerHand {
    Left,
    Right,
}

impl Default for ControllerHand {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerPoseKind {
    /// A pointing pose, typically used for ray-based UI interaction.
    Aim,
    /// A “held object” pose, typically used for attaching models/tools.
    Grip,
}

impl Default for ControllerPoseKind {
    fn default() -> Self {
        Self::Aim
    }
}

/// Marker/config for an XR controller tracked pose.
///
/// Semantics:
/// - Attach a `TransformComponent` as a child of this component.
    /// - the active VR backend will drive that transform child from controller pose tracking.
#[derive(Debug, Clone, Default)]
pub struct ControllerXRComponent {
    pub enabled: bool,
    pub hand: ControllerHand,
    pub pose: ControllerPoseKind,

    // Cached ECS id (runtime-only). Filled during init.
    pub component_id: Option<ComponentId>,
}

impl ControllerXRComponent {
    pub fn new(enabled: bool, hand: ControllerHand, pose: ControllerPoseKind) -> Self {
        Self {
            enabled,
            hand,
            pose,
            component_id: None,
        }
    }

    pub fn on_left_aim() -> Self {
        Self::new(true, ControllerHand::Left, ControllerPoseKind::Aim)
    }

    pub fn on_right_aim() -> Self {
        Self::new(true, ControllerHand::Right, ControllerPoseKind::Aim)
    }
}

impl Component for ControllerXRComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "vr_hand"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterControllerXr {
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
            crate::engine::ecs::IntentValue::RemoveControllerXr {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let hand = match self.hand {
            ControllerHand::Left => "Left",
            ControllerHand::Right => "Right",
        };
        let pose = match self.pose {
            ControllerPoseKind::Aim => "Aim",
            ControllerPoseKind::Grip => "Grip",
        };
        ce_call(
            "VrHand",
            "new",
            vec![b(self.enabled), s(hand), s(pose)],
        )
    }
}
