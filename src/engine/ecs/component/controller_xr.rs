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
    /// Grip position combined with aim orientation from the same XR frame.
    GripAim,
}

impl Default for ControllerPoseKind {
    fn default() -> Self {
        Self::Aim
    }
}

/// Frame-local tracking source selected by the XR runtime for this hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControllerPoseSource {
    #[default]
    None,
    /// Wrist/palm position with orientation synthesized from tracked hand landmarks in the
    /// engine's canonical hand basis.
    WristPalm,
    ControllerAim,
    ControllerGrip,
    ControllerGripAim,
}

/// Marker/config for an XR controller tracked pose.
///
/// Semantics:
/// - Attach a `TransformComponent` as a child of this component.
/// - the active XR runtime will drive that transform child from controller pose tracking.
#[derive(Debug, Clone, Default)]
pub struct ControllerXRComponent {
    /// Current raw OpenXR grip in the nearest transform ancestor's space. Never
    /// synthesized from avatar bones, aim orientation, or a previous XR frame.
    pub(crate) grip_pose_in_parent: Option<crate::engine::transform::TransformTrs>,
    pub enabled: bool,
    /// Runtime-only: true after a valid controller pose was applied this frame.
    pub pose_valid: bool,
    /// Runtime-only source selected for the current frame.
    pub active_pose_source: ControllerPoseSource,
    /// Runtime-only raw controller orientations for alignment diagnostics.
    pub raw_aim_rotation: Option<[f32; 4]>,
    pub raw_grip_rotation: Option<[f32; 4]>,
    pub hand: ControllerHand,
    pub pose: ControllerPoseKind,
    pub laser: bool,
    pub(crate) avatar_laser_warned: bool,

    // Cached ECS id (runtime-only). Filled during init.
    pub component_id: Option<ComponentId>,
}

impl ControllerXRComponent {
    pub fn new(enabled: bool, hand: ControllerHand, pose: ControllerPoseKind) -> Self {
        Self {
            grip_pose_in_parent: None,
            enabled,
            pose_valid: false,
            active_pose_source: ControllerPoseSource::None,
            raw_aim_rotation: None,
            raw_grip_rotation: None,
            hand,
            pose,
            laser: false,
            avatar_laser_warned: false,
            component_id: None,
        }
    }

    pub fn laser(mut self) -> Self {
        self.laser = true;
        self
    }

    pub fn on_left_aim() -> Self {
        Self::new(true, ControllerHand::Left, ControllerPoseKind::Aim)
    }

    pub fn on_right_aim() -> Self {
        Self::new(true, ControllerHand::Right, ControllerPoseKind::Aim)
    }
    /// Copy the current OpenXR grip into engine world space; None means untracked.
    pub fn get_grip_pose(
        &self,
        world: &crate::engine::ecs::World,
    ) -> Option<crate::engine::transform::TransformTrs> {
        if !self.enabled {
            return None;
        }
        let pose = self.grip_pose_in_parent?;
        let parent = Self::parent_frame(world, self.component_id?);
        crate::engine::transform::TransformTrs::from_matrix(crate::utils::math::mat4_mul(
            parent,
            pose.to_matrix().ok()?,
        ))
        .ok()
    }
    pub(crate) fn parent_frame(
        world: &crate::engine::ecs::World,
        id: ComponentId,
    ) -> crate::engine::transform::TransformMatrix {
        let mut parent = world.parent_of(id);
        while let Some(id) = parent {
            if let Some(t) = world.get_component_by_id_as::<super::TransformComponent>(id) {
                return t.transform.matrix_world;
            }
            parent = world.parent_of(id);
        }
        crate::engine::transform::TransformTrs::IDENTITY
            .to_matrix()
            .unwrap()
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
        "xr_hand"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component_id = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterControllerXr {
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
            crate::engine::ecs::IntentValue::RemoveControllerXr {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let hand = match self.hand {
            ControllerHand::Left => "Left",
            ControllerHand::Right => "Right",
        };
        let pose = match self.pose {
            ControllerPoseKind::Aim => "Aim",
            ControllerPoseKind::Grip => "Grip",
            ControllerPoseKind::GripAim => "GripAim",
        };
        let mut expression = ce_call("XRHand", "new", vec![b(self.enabled), s(hand), s(pose)]);
        if self.laser {
            expression = expression.with_call("laser", vec![]);
        }
        expression
    }
}
