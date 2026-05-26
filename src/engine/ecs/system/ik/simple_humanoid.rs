//! Simple humanoid body-follow heuristic for AVC.
//!
//! Replaces the previous spine-FABRIK body-follow path with a planar (XZ)
//! deadzone follow rule applied to `model_root`.  See
//! `docs/task/avatar-control-simple-humanoid-body-follow.md` for the rationale.
//!
//! Topology assumption (set up by `AvatarControlSystem` during init):
//!
//! ```text
//! driven_t                                 ← HMD / InputXR pose driver
//! └── AvatarControlComponent
//!     ├── fixed_head_target (visible head mount)
//!     │     └── J_Bip_C_Head
//!     ├── body_pipeline (yaw-only follow)  ← `body_pipeline_id`
//!     │     └── model_root                 ← `model_root_id` (this system writes its local TRS)
//!     │           └── armature root
//!     │                 └── J_Bip_C_Neck   ← `neck_bone_id` (Phase 2: pinned to rest local TRS)
//!     └── controllers, etc.
//! ```
//!
//! Each tick this system reads `driven_t`'s world XZ position, advances the
//! body anchor toward it with a configurable deadzone + follow rate, and
//! writes `model_root.local.translation` so the model_root world XZ lands at
//! the anchor.  Y is held at the configured rest offset (avatar height);
//! body yaw continues to be handled upstream by `QuatYawFollowComponent` on
//! the body pipeline — this system never touches rotation.
//!
//! Phase 2 (neck): the neck bone's local translation is restored to its rest
//! value each tick if anything else perturbs it.  Without spine FABRIK no
//! system currently writes neck translation, but the pin guards against
//! regressions and animation tracks that include translation channels for
//! the neck.

use crate::engine::ecs::component::{AvatarControlComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::{mat_to_quat, quat_conjugate, quat_rotate_vec3};

#[derive(Debug, Default)]
pub struct SimpleHumanoidSystem;

impl SimpleHumanoidSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        let ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .is_some()
            })
            .collect();

        for id in ids {
            tick_one(id, world, emit, dt_sec);
        }
    }
}

fn tick_one(
    avc_id: ComponentId,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    dt_sec: f32,
) {
    let (
        model_root_id_opt,
        body_pipeline_id_opt,
        mut anchor_xz,
        anchor_initialized,
        deadzone,
        follow_rate,
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
            c.body_anchor_world_xz,
            c.body_anchor_initialized,
            c.body_planar_deadzone,
            c.body_planar_follow_rate,
            c.model_root_local_y,
            c.neck_bone_id,
            c.neck_rest_translation,
        )
    };

    let Some(model_root_id) = model_root_id_opt else { return };

    // Parent of model_root: either body_pipeline (yaw follow on) or AVC (skip_body_pipeline).
    let parent_id = body_pipeline_id_opt.unwrap_or(avc_id);

    // driven_t = parent of AVC — supplies world XZ target.
    let Some(driven_t_id) = world.parent_of(avc_id) else { return };
    let Some(driven_t) = world.get_component_by_id_as::<TransformComponent>(driven_t_id) else {
        return;
    };
    let driven_world = driven_t.transform.matrix_world;
    let driven_x = driven_world[3][0];
    let driven_z = driven_world[3][2];

    // Parent (body_pipeline or AVC) world matrix: yaw rotation + driven_t position.
    let Some(parent_tc) = world.get_component_by_id_as::<TransformComponent>(parent_id) else {
        return;
    };
    let parent_world = parent_tc.transform.matrix_world;
    let parent_pos = [parent_world[3][0], parent_world[3][1], parent_world[3][2]];
    let parent_rot = mat_to_quat(parent_world);
    let inv_parent_rot = quat_conjugate(parent_rot);

    // Initialize anchor on first tick to driven_t.xz so we don't snap-walk on spawn.
    if !anchor_initialized {
        anchor_xz = [driven_x, driven_z];
    }

    // Planar deadzone follow.  When the head/driver leaves the deadzone radius,
    // pull the anchor toward the driver at `follow_rate` m/s, capping at the
    // overshoot distance so we never overshoot the deadzone boundary in one tick.
    let dx = driven_x - anchor_xz[0];
    let dz = driven_z - anchor_xz[1];
    let dist = (dx * dx + dz * dz).sqrt();
    if dist > deadzone && dist > 1e-6 {
        let overshoot = dist - deadzone;
        let step = (follow_rate * dt_sec).min(overshoot);
        let nx = dx / dist;
        let nz = dz / dist;
        anchor_xz[0] += nx * step;
        anchor_xz[1] += nz * step;
    }

    // Persist anchor.
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        c.body_anchor_world_xz = anchor_xz;
        c.body_anchor_initialized = true;
    }

    // Compute model_root.local so model_root.world.xz lands at anchor.xz and
    // world.y stays at driven_t.y + body_local_y.  Y is rotation-invariant
    // under yaw, so we can just feed body_local_y directly.
    let target_world = [
        anchor_xz[0],
        parent_pos[1] + body_local_y,
        anchor_xz[1],
    ];
    let delta = [
        target_world[0] - parent_pos[0],
        target_world[1] - parent_pos[1],
        target_world[2] - parent_pos[2],
    ];
    let local_t = quat_rotate_vec3(inv_parent_rot, delta);

    let (cur_rot, cur_s) = world
        .get_component_by_id_as::<TransformComponent>(model_root_id)
        .map(|t| (t.transform.rotation, t.transform.scale))
        .unwrap_or(([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]));

    emit.push_intent_now(
        model_root_id,
        IntentValue::UpdateTransform {
            component_ids: vec![model_root_id],
            translation: local_t,
            rotation_quat_xyzw: cur_rot,
            scale: cur_s,
        },
    );

    // Phase 2: neck rest-pin.  Restore neck local translation to its cached
    // rest value if anything (e.g. animation track) perturbs it.
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
