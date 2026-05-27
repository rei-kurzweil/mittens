//! Head-pose-sensitive body XZ translate follow for AVC.
//!
//! See `docs/task/avatar-control-simple-humanoid-body-follow.md`, Phase 1.
//!
//! ## v0 baseline (current behavior)
//!
//! Each tick, the body's `model_root.local.translation` is recomputed so
//! that `model_root.world.xz` lands at the **head bone's world XZ** — i.e.
//! wherever the displaced head bone has been driven to by `head_target`
//! under `driven_t` (HMD pose + authored eye offset).  Y stays at the
//! init-time `model_root_local_y` (avatar height calibration).
//!
//! This is the "assume walking" baseline:
//!
//! - if the user walks / leans / steps, the HMD's world XZ moves, the
//!   head bone moves with it (via the `head_target` chain), and the body
//!   follows the head bone 1:1,
//! - if the user only rotates the head, the head bone *also* moves in
//!   world XZ because `head_target` swings the head bone around the HMD
//!   origin by the eye-offset vector — and at v0 the body chases that
//!   motion. v1 will subtract the rotation-induced part to keep the body
//!   put under the *real* neck instead of the head bone.
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
use crate::utils::math::{mat4_mul, mat_to_quat, quat_conjugate, quat_rotate_vec3};

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
    let (
        model_root_id_opt,
        body_pipeline_id_opt,
        head_bone_id_opt,
        body_local_y,
        neck_bone_id_opt,
        neck_rest_t_opt,
    ) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            c.model_root_id,
            c.body_pipeline_id,
            c.displaced_head,
            c.model_root_local_y,
            c.neck_bone_id,
            c.neck_rest_translation,
        )
    };

    let Some(model_root_id) = model_root_id_opt else { return };
    let _ = head_bone_id_opt; // v0: target HMD pose, not displaced head bone.

    // Parent of model_root: body_pipeline (yaw follow) or AVC (skip_body_pipeline).
    let parent_id = body_pipeline_id_opt.unwrap_or(avc_id);

    // Parent world matrix: yaw-rotated body_pipeline output. Its translation
    // passes through from driven_t — i.e. the raw HMD/input-driver pose with
    // NO eye-offset added. This is what we want body XZ to track.
    let Some(parent_world) = world
        .get_component_by_id_as::<TransformComponent>(parent_id)
        .map(|t| t.transform.matrix_world)
    else {
        return;
    };
    let parent_pos = [parent_world[3][0], parent_world[3][1], parent_world[3][2]];
    let parent_rot = mat_to_quat(parent_world);
    let inv_parent_rot = quat_conjugate(parent_rot);

    // Target = HMD world XZ (= parent_pos.xz, since body_pipeline passes
    // driven_t translation through). Targeting displaced_head.world.xz
    // instead introduces a fixed eye-offset lag because the head bone is a
    // descendant of model_root displaced by +eye_offset via head_target.
    let target_world = [parent_pos[0], parent_pos[1] + body_local_y, parent_pos[2]];
    let delta = [
        target_world[0] - parent_pos[0],
        target_world[1] - parent_pos[1],
        target_world[2] - parent_pos[2],
    ];
    let new_local_t = quat_rotate_vec3(inv_parent_rot, delta);

    // Write directly to model_root.transform — see the doc comment at the
    // top of the file for why we bypass `IntentValue::UpdateTransform`.
    if let Some(tc) = world.get_component_by_id_as_mut::<TransformComponent>(model_root_id) {
        tc.transform.translation = new_local_t;
        tc.transform.recompute_model();
        tc.transform.matrix_world = mat4_mul(parent_world, tc.transform.model);
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

