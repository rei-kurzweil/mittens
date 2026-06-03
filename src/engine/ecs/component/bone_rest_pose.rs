use crate::engine::ecs::component::Component;
use crate::engine::ecs::ComponentId;

/// Immutable snapshot of a bone's local TRS at GLTF load time.
///
/// Animation, IK, and post-load systems all overwrite `TransformComponent`
/// during the tick, so by the time `AvatarControlSystem::try_init_splices`
/// runs the bone's `TransformComponent` no longer holds the authored bind
/// pose.  This component is attached as a child of each GLTF-spawned bone
/// `TransformComponent` so consumers can look up the *true* rest local
/// translation / rotation / scale without depending on tick ordering.
///
/// Created once by `GLTFSystem` during node spawn and never written again.
///
/// Lookup pattern:
/// ```ignore
/// let rest = world
///     .children_of(bone_tc_id)
///     .iter()
///     .find_map(|&c| world.get_component_by_id_as::<BoneRestPoseComponent>(c));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BoneRestPoseComponent {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],

    component: Option<ComponentId>,
}

impl BoneRestPoseComponent {
    pub fn new(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Self {
        Self {
            translation,
            rotation,
            scale,
            component: None,
        }
    }
}

impl Component for BoneRestPoseComponent {
    fn name(&self) -> &'static str {
        "bone_rest_pose"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce("BoneRestPose").with_call(
            "translation",
            vec![
                num(self.translation[0] as f64),
                num(self.translation[1] as f64),
                num(self.translation[2] as f64),
            ],
        )
    }
}
