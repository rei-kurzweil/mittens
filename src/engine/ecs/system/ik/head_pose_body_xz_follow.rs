//! Head-pose-sensitive body XZ translate follow for AVC.
//!
//! See `docs/task/avatar-control-simple-humanoid-body-follow.md`, Phase 1.
//!
//! ## v0 baseline (current behavior)
//!
//! Each tick, the body's `model_root.local.translation` is recomputed so
//! that `model_root.world.xz` lands at the **displaced head bone's world
//! XZ** — i.e. the skull-base / head-pivot point the head solver itself
//! treats as the anchor. Y stays at the init-time `model_root_local_y`
//! (avatar height calibration), and body yaw still comes from the
//! yaw-follow pipeline.
//!
//! `displaced_head` is re-parented under `driven_t -> head_target` at AVC
//! init (see `avatar_control_system.rs`), so its world transform is a pure
//! function of the HMD pose (HMD pos rotated by -eye_offset). It is NOT a
//! descendant of `model_root`, so writing the body translation here does
//! not feed back into the target next tick.
//!
//! Targeting the HMD center instead would put the body under the eye
//! position, leaving the neck pivot behind the body and causing the neck
//! to stretch/lean under head rotation. See
//! `docs/bugs/avatar-body-follow-targets-hmd-center-instead-of-head-pivot-xz.md`.
//!
//! ## Why we write `matrix_world` ourselves instead of `UpdateTransform`
//!
//! `IntentValue::UpdateTransform` builds a fresh `Transform` and assigns
//! it to the `TransformComponent`, which resets `matrix_world` to identity
//! before `transform_changed` runs.  For a normal TC the propagation walk
//! would then rebuild `matrix_world` from the parent chain.  For
//! `model_root`, the parent is `body_pipeline` (a transform-stream
//! boundary) and `transform_changed` deliberately *skips* recomputing
//! `model_root.matrix_world` from local TRS — it assumes the cached value
//! is stream-managed (`src/engine/ecs/system/transform_system.rs:253-296`).
//! Result: `matrix_world` stays at identity and the body teleports to
//! origin every tick.
//!
//! So we directly mutate `model_root.transform` (translation, model,
//! matrix_world) and then emit `UpdateTransformWorld` to trigger child
//! propagation — that intent calls `transform_changed` without resetting
//! the cached world matrix.

use crate::engine::ecs::component::{AvatarControlComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::{mat_to_quat, quat_conjugate, quat_rotate_vec3};

#[derive(Debug, Default)]
pub struct HeadPoseBodyXzFollowSystem;

impl HeadPoseBodyXzFollowSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, _dt_sec: f32) {
        let ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .is_some()
            })
            .collect();

        for id in ids {
            tick_one(id, world, emit);
        }
    }
}

fn tick_one(avc_id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter) {
    let (model_root_id_opt, body_local_y, head_bone_id_opt, neck_bone_id_opt, neck_rest_t_opt) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            c.model_root_id,
            c.model_root_local_y,
            c.displaced_head,
            c.neck_bone_id,
            c.neck_rest_translation,
        )
    };

    let Some(model_root_id) = model_root_id_opt else {
        return;
    };
    let Some(head_bone_id) = head_bone_id_opt else {
        return;
    };
    if !xr_source_is_valid(world, head_bone_id) {
        return;
    }

    // Target = displaced head bone world XZ (skull-base / head-pivot).
    // `displaced_head` lives under driven_t->head_target, so this is a
    // function of HMD pose only — no feedback from writing model_root below.
    let Some(head_world) = world
        .get_component_by_id_as::<TransformComponent>(head_bone_id)
        .map(|t| t.transform.matrix_world)
    else {
        return;
    };

    // Preserve current world basis (yaw-follow owns rotation); only retarget
    // world translation. Y stays at HMD y + body_local_y for height.
    let Some(current_world) = world
        .get_component_by_id_as::<TransformComponent>(model_root_id)
        .map(|t| t.transform.matrix_world)
    else {
        return;
    };

    let mut next_world = current_world;
    next_world[3][0] = head_world[3][0];
    next_world[3][1] = head_world[3][1] + body_local_y;
    next_world[3][2] = head_world[3][2];

    // Recover the implied local translation for the cached `translation` field
    // (model_root's parent is body_pipeline / AVC; rotation only, so invert
    // current world rot to map world-delta back to local).
    let current_rot = mat_to_quat(current_world);
    let inv_rot = quat_conjugate(current_rot);
    let world_delta = [
        next_world[3][0] - current_world[3][0],
        next_world[3][1] - current_world[3][1],
        next_world[3][2] - current_world[3][2],
    ];
    let local_delta = quat_rotate_vec3(inv_rot, world_delta);
    let prev_local_t = world
        .get_component_by_id_as::<TransformComponent>(model_root_id)
        .map(|t| t.transform.translation)
        .unwrap_or([0.0, 0.0, 0.0]);
    let new_local_t = [
        prev_local_t[0] + local_delta[0],
        prev_local_t[1] + local_delta[1],
        prev_local_t[2] + local_delta[2],
    ];

    // Write directly to model_root.transform — see the doc comment at the
    // top of the file for why we bypass `IntentValue::UpdateTransform`.
    if let Some(tc) = world.get_component_by_id_as_mut::<TransformComponent>(model_root_id) {
        tc.transform.translation = new_local_t;
        tc.transform.recompute_model();
        tc.transform.matrix_world = next_world;
    }

    // Trigger child propagation: `UpdateTransformWorld` calls
    // `transform_changed` without resetting `matrix_world`, so the value
    // we just wrote above survives and propagation to descendants uses it.
    emit.push_intent_now(
        model_root_id,
        IntentValue::UpdateTransformWorld {
            component_ids: vec![model_root_id],
        },
    );

    // Phase 2 neck rest-pin.
    if let (Some(neck_id), Some(neck_rest_t)) = (neck_bone_id_opt, neck_rest_t_opt) {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(neck_id) {
            let cur = t.transform.translation;
            let drift = (cur[0] - neck_rest_t[0]).abs()
                + (cur[1] - neck_rest_t[1]).abs()
                + (cur[2] - neck_rest_t[2]).abs();
            if drift > 1e-5 {
                let rot = t.transform.rotation;
                let scl = t.transform.scale;
                emit.push_intent_now(
                    neck_id,
                    IntentValue::UpdateTransform {
                        component_ids: vec![neck_id],
                        translation: neck_rest_t,
                        rotation_quat_xyzw: rot,
                        scale: scl,
                    },
                );
            }
        }
    }
}

fn xr_source_is_valid(world: &World, start: ComponentId) -> bool {
    let mut current = Some(start);
    while let Some(component) = current {
        if let Some(input) = world
            .get_component_by_id_as::<crate::engine::ecs::component::InputXRComponent>(component)
        {
            return input.pose_valid;
        }
        current = world.parent_of(component);
    }
    true
}
