mod eye_blink;
mod arm_ik;
mod capsule;
mod eyes;
mod hands;
mod init;
mod mouth;
mod topology;
use eye_blink::*;
use arm_ik::*;
use capsule::*;
use eyes::*;
use hands::*;
use init::*;
use mouth::*;
use topology::*;
use super::ik::rest_pose::{read_bone_rest_pose, rest_model_relative_to};
use crate::engine::ecs::component::HumanoidSlot;
use crate::engine::ecs::component::{
    AvatarControlComponent, BoneRestPoseComponent, Camera3DComponent, CameraXRComponent,
    CollidableComponent, CollisionResponse, CollisionShape, ControllerHand, ControllerPoseSource,
    ControllerXRComponent, EyeRotationLimits, GLTFComponent, HeadMotionGazePolicy,
    HeadRotationCompensation, IKChainComponent, IKSolver, InputComponent, InputXRComponent,
    SerializeComponent, TransformComponent, TransformOperatorComponent,
    VRChatOSCEyeTrackingComponent, XREyeTrackingComponent, XREyeTrackingHtcComponent,
    ZoneComponent,
};
use crate::engine::ecs::component::{
    QueryRootMode, is_level_provider, resolve_component_ref, retained_level_sample,
};
use crate::engine::ecs::system::bounds_system::{BoundsSystem, RenderableBoundsMeasure};
use crate::engine::ecs::system::collision_shape_inference::infer_upright_capsule;
use crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform;
use crate::engine::ecs::system::{
    HumanoidBoneMapReport, HumanoidBoneMapSystem, JointBasisRetargetingSystem, LandmarkDirection,
    RetargetBasisDefinition, RetargetBasisStatus,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;
use crate::engine::user_input::InputState;
use crate::utils::math::{
    mat_to_quat, mat4_identity, mat4_mul, mat4_mul_vec4, quat_conjugate, quat_mul,
    quat_rotate_vec3, quat_rotation_y, quat_to_axis_angle, shortest_arc_quat,
};
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct AvatarControlSystem {
    avatars: HashSet<ComponentId>,
    pending_capsule_diagnostics: HashSet<ComponentId>,
    alignment_diagnostic_tick: u64,
}

impl AvatarControlSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        _input: &InputState,
        render_assets: &RenderAssets,
        retargeting: &mut JointBasisRetargetingSystem,
        humanoid_maps: &mut HumanoidBoneMapSystem,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
        operators: &super::TransformOperatorSystem,
    ) {
        let ids: Vec<_> = self.avatars.iter().copied().collect();

        self.alignment_diagnostic_tick = self.alignment_diagnostic_tick.wrapping_add(1);
        let log_alignment =
            hand_alignment_debug_enabled() && self.alignment_diagnostic_tick % 120 == 0;
        for id in ids {
            if self.pending_capsule_diagnostics.remove(&id) {
                log_settled_capsule_diagnostics(id, world);
            }
            let capsule_before = world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .and_then(|avc| avc.capsule_transform_id);
            tick_one(
                id,
                world,
                render_assets,
                retargeting,
                humanoid_maps,
                emit,
                dt_sec,
                log_alignment,
            );
            let capsule_after = world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .and_then(|avc| avc.capsule_transform_id);
            if capsule_before.is_none() && capsule_after.is_some() {
                self.pending_capsule_diagnostics.insert(id);
            }
            // A new capsule must pass through transform propagation before it
            // can release scene-authored gravity. Publish on a subsequent tick.
            if capsule_before.is_some() {
                publish_capsule_readiness(id, world, emit, operators);
            }
        }
    }

    /// Retained avatar IDs bound this work; no world-wide avatar search.
    pub fn pending_capsule_samples(
        &self,
        world: &mut World,
        operators: &super::TransformOperatorSystem,
    ) -> Vec<ComponentId> {
        let mut pending = Vec::new();
        for &id in &self.avatars {
            refresh_capsule_routing(id, world);
            if let Some(sample) = world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .and_then(|avc| avc.capsule_sample_id)
            {
                if operators.translation_sample_status(world, sample)
                    == super::transform_operator::TranslationSampleStatus::Pending
                {
                    pending.push(sample);
                }
            }
        }
        pending
    }

    pub fn gate_capsule_contact(
        &self,
        world: &mut World,
        operators: &super::TransformOperatorSystem,
        contact: &mut super::static_contact_system::StaticContactSystem,
    ) {
        for &id in &self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            let enabled = capsule_contact_valid(world, avatar, operators);
            let response = avatar.collision_response;
            let collidable = avatar.capsule_collidable_id;
            let reset = avatar.capsule_reset_contact_history || collidable
                .and_then(|id| world.get_component_by_id_as::<CollidableComponent>(id))
                .is_some_and(|c| c.response != response);
            if reset {
                if let Some(zone) = collidable.and_then(|id| world.parent_of(id)) {
                    contact.forget_zone(zone);
                }
                world
                    .get_component_by_id_as_mut::<AvatarControlComponent>(id)
                    .unwrap()
                    .capsule_reset_contact_history = false;
            }
            if let Some(collidable) = collidable
                .and_then(|id| world.get_component_by_id_as_mut::<CollidableComponent>(id))
            {
                collidable.enabled = enabled;
                // Permission changes take effect before any contact pass, even
                // when route preparation occurs in a later avatar phase.
                collidable.response = response;
            }
        }
    }

    pub fn register(&mut self, component: ComponentId) {
        self.avatars.insert(component);
    }

    pub fn remove(&mut self, component: ComponentId) {
        self.avatars.remove(&component);
        self.pending_capsule_diagnostics.remove(&component);
    }
}

fn tick_one(
    id: ComponentId,
    world: &mut World,
    render_assets: &RenderAssets,
    retargeting: &mut JointBasisRetargetingSystem,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    emit: &mut dyn SignalEmitter,
    dt_sec: f32,
    log_alignment: bool,
) {
    // Shared rig binding is independent of tracked head embodiment. An
    // untracked NPC keeps its authored model transform and bone hierarchy.
    let has_pose_driver = owning_input_xr(world, id).is_some()
        || owning_desktop_input(world, id).is_some()
        || owning_pose_input(world, id);
    if !has_pose_driver {
        if let Some(gltf) = humanoid_maps.request_for_avc(world, id)
            && let Some(report) = humanoid_maps.report(gltf)
            && let Some(root) = model_root(world, id)
        {
            bind_shared_rig(world, id, root, report);
        }
    } else {
        let needs_init = world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .is_some_and(|avatar| avatar.head_mount.is_none());
        if needs_init {
            if !ancestor_input_xr_is_ready(world, id) {
                return;
            }
            let Some(gltf) = humanoid_maps.request_for_avc(world, id) else {
                return;
            };
            let Some(report) = humanoid_maps.report(gltf).cloned() else {
                return;
            };
            if !report.head_ready() {
                return;
            }
            try_init_pose_drivers(id, world, retargeting, &report, emit);
        }
    }

    refresh_map_camera_anchor(id, world, humanoid_maps, emit);

    try_init_or_route_capsule(id, world, render_assets, emit);

    // Keep the displaced head bone anchored under head_mount. This prevents
    // animation/FK from reintroducing a local head translation that would move
    // the camera wrapper relative to the solved head pivot.
    let displaced_head_id = world
        .get_component_by_id_as::<AvatarControlComponent>(id)
        .and_then(|c| c.displaced_head);
    if let Some(head_bone_id) = displaced_head_id {
        if let Some(head_t) = world.get_component_by_id_as::<TransformComponent>(head_bone_id) {
            if head_t.transform.translation != [0.0, 0.0, 0.0] {
                emit.push_intent_now(
                    head_bone_id,
                    IntentValue::UpdateTransform {
                        component_id: head_bone_id,
                        translation: [0.0, 0.0, 0.0],
                        rotation_quat_xyzw: head_t.transform.rotation,
                        scale: head_t.transform.scale,
                    },
                );
            }
        }
    }

    update_hand_pose_corrections(id, world, emit, log_alignment);
    update_eye_tracking(id, world, humanoid_maps, emit, dt_sec);
    update_amplitude_mouth_open(id, world, humanoid_maps, dt_sec);
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod first_person_test_support;
