//! Legacy body follow, retained only as the native constraint's fallback.
//!
//! See `docs/task/avatar-control-simple-humanoid-body-follow.md`, Phase 1.
//!
//! ## v0 baseline (current behavior)
//!
//! Each tick, the body's `model_root.local.translation` is recomputed so
//! that `model_root.world.xz` lands at the **displaced head bone's world
//! XZ** — i.e. the skull-base / head-pivot point. Y follows that pivot plus
//! the init-time `model_root_local_y` height calibration. Body yaw still
//! comes from the yaw-follow operator.
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
//! `model_root`, the parent is `body_operator` (a transform operator
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

pub(super) fn tick_one(
    avc_id: ComponentId,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    restored_head_world: Option<crate::engine::transform::TransformMatrix>,
) {
    let (model_root_id_opt, body_local_y, head_bone_id_opt, neck_bone_id_opt, neck_rest_t_opt) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            c.model_root_id,
            c.model_root_local_y,
            c.displaced_head,
            c.neck_bone_id,
            c.neck_rest_translation.filter(|_| c.neck_pin_enabled),
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
    let unspliced = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .and_then(|avatar| {
            Some((
                avatar.head_mount?,
                avatar.head_neck_body_constraint.as_ref()?.head_rest(),
            ))
        })
        .filter(|(mount, _)| world.parent_of(head_bone_id) != Some(*mount));
    let Some(head_world) = restored_head_world.or_else(|| {
        if let Some((mount, rest)) = unspliced {
            let mount = world
                .get_component_by_id_as::<TransformComponent>(mount)?
                .transform
                .matrix_world;
            let local = crate::engine::transform::TransformTrs::new(
                [0.0; 3],
                rest.rotation_quat_xyzw,
                rest.scale,
            )
            .to_matrix()
            .ok()?;
            return Some(crate::utils::math::mat4_mul(mount, local));
        }
        world
            .get_component_by_id_as::<TransformComponent>(head_bone_id)
            .map(|t| t.transform.matrix_world)
    }) else {
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
    // (model_root's parent is body_operator / AVC; rotation only, so invert
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
            component_id: model_root_id,
        },
    );

    if let Some((_, rest)) = unspliced
        && let Some(parent) = world.parent_of(head_bone_id)
        && let Some(parent_world) = super::super::TransformSystem::world_model(world, parent)
    {
        // Keep mouse look authoritative even when native neck bending is opted
        // out. The virtual target, not the posed head, drives legacy placement.
        let rotation = crate::utils::math::quat_mul(
            quat_conjugate(mat_to_quat(parent_world)),
            mat_to_quat(head_world),
        );
        emit.push_intent_now(
            head_bone_id,
            IntentValue::UpdateTransform {
                component_id: head_bone_id,
                translation: rest.translation,
                rotation_quat_xyzw: rotation,
                scale: rest.scale,
            },
        );
    }

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
                        component_id: neck_id,
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
