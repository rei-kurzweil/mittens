//! Native, immediate-follow head/neck/body constraint. VMN is not involved.
//!
//! AVC captures the original FK segment before moving the head to its input
//! mount. The neck bends from its rest pose, and body placement closes that
//! segment against the authoritative XR head or the connected desktop target.

use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    AvatarControlComponent, InputComponent, InputXRComponent, TransformComponent,
};
use crate::engine::ecs::system::AttachmentSystem;
use crate::engine::ecs::system::avatar_control_system::{
    read_bone_rest_pose, rest_model_relative_to,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformMatrix, TransformTrs};
use crate::utils::math::{
    mat4_inverse, mat4_mul, mat4_mul_vec4, quat_conjugate, quat_from_axis_angle, quat_mul,
    quat_rotate_vec3, vec3_add, vec3_len, vec3_sub,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadNeckBodyConstraintStatus {
    Ready,
    Pending,
    Disabled,
    Mounted,
    InvalidPose,
    UnsupportedRig(&'static str),
}

/// Per-avatar calibration, captured before the head's original FK path is lost.
/// Runtime-only: never serialized or shared between avatar instances.
#[derive(Debug, Clone)]
pub struct HeadNeckBodyConstraint {
    neck: ComponentId,
    head: ComponentId,
    root: ComponentId,
    driven: ComponentId,
    neck_parent: ComponentId,
    neck_attachment_parent: Option<ComponentId>,
    neck_ancestors: Vec<(ComponentId, Option<ComponentId>)>,
    neck_rest: TransformTrs,
    neck_to_head: [f32; 3],
    desktop_anchor_offset: [f32; 3],
    mount_rest_translation: [f32; 3],
    pub(crate) map_generation: u64,
    active: bool,
}

fn uniform(matrix: TransformMatrix) -> Result<TransformTrs, &'static str> {
    let trs = TransformTrs::from_matrix(matrix).map_err(|_| "invalid or sheared transform")?;
    let s = trs.scale;
    let tolerance = s[0].abs().max(1.0) * 0.0001;
    if s[0] <= 0.0 || (s[0] - s[1]).abs() > tolerance || (s[0] - s[2]).abs() > tolerance {
        return Err("nonuniform scale");
    }
    Ok(trs)
}

fn matrix(world: &World, id: ComponentId) -> Option<TransformMatrix> {
    Some(
        world
            .get_component_by_id_as::<TransformComponent>(id)?
            .transform
            .matrix_world,
    )
}

fn point(matrix: TransformMatrix, p: [f32; 3]) -> [f32; 3] {
    let p = mat4_mul_vec4(matrix, [p[0], p[1], p[2], 1.0]);
    [p[0], p[1], p[2]]
}

impl HeadNeckBodyConstraint {
    pub(crate) fn bind(
        world: &World,
        root: ComponentId,
        neck: ComponentId,
        head: ComponentId,
        driven: ComponentId,
        mount_rest_translation: [f32; 3],
        initial_body_rotation: [f32; 4],
        map_generation: u64,
    ) -> Result<Self, &'static str> {
        rest_model_relative_to(world, root, neck).ok_or("neck is not below model root")?;
        let mut neck_ancestors = Vec::new();
        let mut node = neck;
        while node != root {
            let parent = world.parent_of(node);
            neck_ancestors.push((node, parent));
            node = parent.ok_or("neck is not below model root")?;
        }
        let head_in_neck =
            rest_model_relative_to(world, neck, head).ok_or("head is not below mapped neck")?;
        uniform(head_in_neck)?;
        let neck_to_head = head_in_neck[3][..3].try_into().unwrap();
        if vec3_len(neck_to_head) < 0.0001 {
            return Err("zero neck-to-head segment");
        }
        let head_in_root =
            rest_model_relative_to(world, root, head).ok_or("head is not below model root")?;
        uniform(head_in_root)?;
        let (translation, rotation, scale) = read_bone_rest_pose(world, neck);
        let neck_rest = TransformTrs::new(translation, rotation, scale)
            .normalized()
            .map_err(|_| "invalid neck rest pose")?;
        uniform(
            neck_rest
                .to_matrix()
                .map_err(|_| "invalid neck rest pose")?,
        )?;
        let mut parent = world.parent_of(neck);
        let neck_parent = loop {
            let id = parent.ok_or("neck has no transform parent")?;
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_some()
            {
                break id;
            }
            parent = world.parent_of(id);
        };
        let head_rest_position = head_in_root[3][..3].try_into().unwrap();
        let driven_t = world
            .get_component_by_id_as::<TransformComponent>(driven)
            .ok_or("missing pose driver transform")?;
        let driver_parent = mat4_mul(
            driven_t.transform.matrix_world,
            mat4_inverse(driven_t.transform.model).ok_or("singular pose driver")?,
        );
        let parent_rotation = uniform(driver_parent)?.rotation_quat_xyzw;
        let body_rotation_in_driver_parent =
            quat_mul(quat_conjugate(parent_rotation), initial_body_rotation);
        let desktop_anchor_offset = vec3_sub(
            mount_rest_translation,
            quat_rotate_vec3(body_rotation_in_driver_parent, head_rest_position),
        );
        Ok(Self {
            neck,
            head,
            root,
            driven,
            neck_parent,
            neck_attachment_parent: world.parent_of(neck),
            neck_ancestors,
            neck_rest,
            neck_to_head,
            desktop_anchor_offset,
            mount_rest_translation,
            map_generation,
            active: false,
        })
    }

    pub(crate) fn matches_map(&self, head: Option<ComponentId>, neck: Option<ComponentId>) -> bool {
        head == Some(self.head) && neck == Some(self.neck)
    }

    fn candidate(
        &self,
        world: &World,
        avatar: &AvatarControlComponent,
    ) -> Result<Candidate, HeadNeckBodyConstraintStatus> {
        use HeadNeckBodyConstraintStatus::{InvalidPose, UnsupportedRig};
        if self.map_generation != avatar.humanoid_map_generation
            || world.parent_of(self.neck) != self.neck_attachment_parent
            || self
                .neck_ancestors
                .iter()
                .any(|&(id, parent)| world.parent_of(id) != parent)
            || avatar.model_root_id != Some(self.root)
            || avatar.displaced_head != Some(self.head)
        {
            return Err(UnsupportedRig("rig binding changed"));
        }
        if !avatar.neck_pitch_share.is_finite()
            || !(0.0..=1.0).contains(&avatar.neck_pitch_share)
            || !avatar.neck_extension_limit.is_finite()
            || !(0.0..=std::f32::consts::FRAC_PI_2).contains(&avatar.neck_extension_limit)
            || !avatar.neck_flexion_limit.is_finite()
            || !(0.0..=std::f32::consts::FRAC_PI_2).contains(&avatar.neck_flexion_limit)
            || !avatar.neck_roll_share.is_finite()
            || !(0.0..=1.0).contains(&avatar.neck_roll_share)
            || !avatar.neck_lateral_limit.is_finite()
            || !(0.0..=std::f32::consts::FRAC_PI_2).contains(&avatar.neck_lateral_limit)
        {
            return Err(InvalidPose);
        }
        let root_world = matrix(world, self.root).ok_or(InvalidPose)?;
        let root_trs = uniform(root_world).map_err(UnsupportedRig)?;
        let parent_world = matrix(world, self.neck_parent).ok_or(InvalidPose)?;
        let parent_in_root = mat4_mul(mat4_inverse(root_world).ok_or(InvalidPose)?, parent_world);
        let parent_trs = uniform(parent_in_root).map_err(UnsupportedRig)?;
        let mount = avatar.head_mount.ok_or(InvalidPose)?;
        let mount_world =
            uniform(matrix(world, mount).ok_or(InvalidPose)?).map_err(UnsupportedRig)?;
        let head_in_body = quat_mul(
            quat_conjugate(root_trs.rotation_quat_xyzw),
            mount_world.rotation_quat_xyzw,
        );
        let bend = preferred_neck_bend(head_in_body, avatar);
        let neck_rotation = preferred_neck_rotation(
            parent_trs.rotation_quat_xyzw,
            self.neck_rest.rotation_quat_xyzw,
            bend,
        );
        let neck_local = TransformTrs::new(
            self.neck_rest.translation,
            neck_rotation,
            self.neck_rest.scale,
        );
        let virtual_neck = mat4_mul(
            parent_in_root,
            neck_local.to_matrix().map_err(|_| InvalidPose)?,
        );
        let endpoint = point(virtual_neck, self.neck_to_head);
        // Strip root translation when transforming the root-relative FK endpoint.
        let displacement = vec3_sub(point(root_world, endpoint), root_trs.translation);
        let driven_world = matrix(world, self.driven).ok_or(InvalidPose)?;
        let driven = world
            .get_component_by_id_as::<TransformComponent>(self.driven)
            .ok_or(InvalidPose)?;
        TransformTrs::new(
            driven.transform.translation,
            driven.transform.rotation,
            driven.transform.scale,
        )
        .normalized()
        .map_err(|_| InvalidPose)?;
        uniform(driven_world).map_err(UnsupportedRig)?;
        let desktop = desktop_source(world, self.driven)?;
        let (head_position, mount_local) = if desktop {
            // Input translation/its outer frame own locomotion. Looking changes
            // only the head arc, never feeds a derived camera pose back to Input.
            let driver_parent = mat4_mul(
                driven_world,
                mat4_inverse(driven.transform.model).ok_or(InvalidPose)?,
            );
            let scaled_anchor_offset =
                std::array::from_fn(|i| self.desktop_anchor_offset[i] * driven.transform.scale[i]);
            let anchor = point(
                driver_parent,
                vec3_add(driven.transform.translation, scaled_anchor_offset),
            );
            let desired_head = vec3_add(anchor, displacement);
            let mount_position =
                point(mat4_inverse(driven_world).ok_or(InvalidPose)?, desired_head);
            let mount_t = world
                .get_component_by_id_as::<TransformComponent>(mount)
                .ok_or(InvalidPose)?;
            (
                desired_head,
                Some((
                    mount,
                    TransformTrs::new(
                        mount_position,
                        mount_t.transform.rotation,
                        mount_t.transform.scale,
                    ),
                )),
            )
        } else {
            // Reconstruct the raw AVC head mount, so switching from desktop
            // never treats the prior derived head arc as a measured XR sample.
            let mount_t = world
                .get_component_by_id_as::<TransformComponent>(mount)
                .ok_or(InvalidPose)?;
            let raw_mount = TransformTrs::new(
                self.mount_rest_translation,
                mount_t.transform.rotation,
                mount_t.transform.scale,
            );
            let head_local = world
                .get_component_by_id_as::<TransformComponent>(self.head)
                .ok_or(InvalidPose)?
                .transform
                .model;
            let head_world = mat4_mul(
                mat4_mul(
                    driven_world,
                    raw_mount.to_matrix().map_err(|_| InvalidPose)?,
                ),
                head_local,
            );
            let head_position = head_world[3][..3].try_into().unwrap();
            let restore_mount = (mount_t.transform.translation != self.mount_rest_translation)
                .then_some((mount, raw_mount));
            (head_position, restore_mount)
        };
        // Both input adapters produce a desired head pose. The same geometric
        // closure places the body beneath that target, regardless of its source.
        let body_position = vec3_sub(head_position, displacement);
        let root = world
            .get_component_by_id_as::<TransformComponent>(self.root)
            .ok_or(InvalidPose)?;
        // Recover the real effective operator basis from its evaluated output.
        // This handles yaw, rotated parents and uniform scale without walking
        // structural ancestry past an operator or accumulating previous deltas.
        let basis = mat4_mul(
            root_world,
            mat4_inverse(root.transform.model).ok_or(InvalidPose)?,
        );
        let root_position = point(mat4_inverse(basis).ok_or(InvalidPose)?, body_position);
        let root_local =
            TransformTrs::new(root_position, root.transform.rotation, root.transform.scale);
        let mut next_world = root_world;
        next_world[3][..3].copy_from_slice(&body_position);
        if !root_local.is_finite()
            || !neck_local.is_finite()
            || mount_local.is_some_and(|(_, t)| !t.is_finite())
        {
            return Err(InvalidPose);
        }
        Ok(Candidate {
            root: self.root,
            root_local,
            root_world: next_world,
            neck: self.neck,
            neck_local,
            mount_local,
        })
    }

    fn restore(
        &mut self,
        world: &World,
        avatar: &AvatarControlComponent,
        emit: &mut dyn SignalEmitter,
    ) -> Option<TransformMatrix> {
        if !self.active {
            return None;
        }
        if world
            .get_component_by_id_as::<TransformComponent>(self.neck)
            .is_some()
        {
            emit_local(emit, self.neck, self.neck_rest);
        }
        let mut raw_head_world = None;
        if let Some((mount, transform)) = avatar.head_mount.and_then(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(id)
                .map(|t| (id, &t.transform))
        }) {
            let raw_mount = TransformTrs::new(
                self.mount_rest_translation,
                transform.rotation,
                transform.scale,
            );
            if let Some(driven_world) = matrix(world, self.driven)
                && let Ok(raw_mount_matrix) = raw_mount.to_matrix()
                && let Some(head) = world.get_component_by_id_as::<TransformComponent>(self.head)
            {
                raw_head_world = Some(mat4_mul(
                    mat4_mul(driven_world, raw_mount_matrix),
                    head.transform.model,
                ));
            }
            emit_local(emit, mount, raw_mount);
        }
        self.active = false;
        raw_head_world
    }
}

fn preferred_neck_rotation(
    parent_rotation: [f32; 4],
    rest_rotation: [f32; 4],
    bend: [f32; 4],
) -> [f32; 4] {
    // Express a body-frame bend in the imported joint's parent frame.
    quat_mul(
        quat_mul(quat_conjugate(parent_rotation), bend),
        quat_mul(parent_rotation, rest_rotation),
    )
}

fn preferred_neck_bend(head: [f32; 4], avatar: &AvatarControlComponent) -> [f32; 4] {
    // Swing/twist around anatomical +Y: head = yaw * tilt. Pure yaw has no
    // swing. Unlike a forward-vector pitch measurement, the swing also retains
    // lateral tilt and remains defined when looking straight up/down.
    let yaw_norm = head[1].hypot(head[3]);
    let yaw = if yaw_norm > 0.000001 {
        [0.0, head[1] / yaw_norm, 0.0, head[3] / yaw_norm]
    } else {
        // At exactly upside-down, yaw is underdetermined; choose zero twist.
        [0.0, 0.0, 0.0, 1.0]
    };
    let tilt = quat_mul(quat_conjugate(yaw), head);
    let planar_len = tilt[0].hypot(tilt[2]);
    if planar_len < 0.000001 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    // q and -q describe the same pose. Select the shortest tilt consistently.
    let sign = if tilt[3] < 0.0 { -1.0 } else { 1.0 };
    let angle = 2.0 * planar_len.atan2(tilt[3].abs());
    let pitch = (sign * tilt[0] / planar_len * angle * avatar.neck_pitch_share)
        .clamp(-avatar.neck_extension_limit, avatar.neck_flexion_limit);
    let roll = (sign * tilt[2] / planar_len * angle * avatar.neck_roll_share)
        .clamp(-avatar.neck_lateral_limit, avatar.neck_lateral_limit);
    let bend_len = pitch.hypot(roll);
    if bend_len < 0.000001 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let bend = quat_from_axis_angle(
        [pitch / bend_len, 0.0, roll / bend_len],
        bend_len.min(std::f32::consts::FRAC_PI_2),
    );
    // Apply tilt in the current head heading, then express it in the body frame.
    // This does not apply the yaw itself to the neck or introduce a yaw owner.
    quat_mul(quat_mul(yaw, bend), quat_conjugate(yaw))
}

fn desktop_source(
    world: &World,
    driven: ComponentId,
) -> Result<bool, HeadNeckBodyConstraintStatus> {
    let mut current = world.parent_of(driven);
    while let Some(id) = current {
        if let Some(input) = world.get_component_by_id_as::<InputXRComponent>(id) {
            return if input.enabled && input.pose_valid {
                Ok(false)
            } else {
                Err(HeadNeckBodyConstraintStatus::Pending)
            };
        }
        if let Some(input) = world.get_component_by_id_as::<InputComponent>(id) {
            return if input.enabled {
                Ok(true)
            } else {
                Err(HeadNeckBodyConstraintStatus::Pending)
            };
        }
        current = world.parent_of(id);
    }
    // A custom/native pose driver supplies an authoritative head target too.
    Ok(false)
}

struct Candidate {
    root: ComponentId,
    root_local: TransformTrs,
    root_world: TransformMatrix,
    neck: ComponentId,
    neck_local: TransformTrs,
    mount_local: Option<(ComponentId, TransformTrs)>,
}

fn emit_local(emit: &mut dyn SignalEmitter, id: ComponentId, trs: TransformTrs) {
    emit.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_id: id,
            translation: trs.translation,
            rotation_quat_xyzw: trs.rotation_quat_xyzw,
            scale: trs.scale,
        },
    );
}

impl Candidate {
    fn apply(self, world: &mut World, emit: &mut dyn SignalEmitter) {
        // First T under a fork is operator-owned: preserve its evaluated world
        // basis and use UpdateTransformWorld to propagate the corrected root.
        let root = world
            .get_component_by_id_as_mut::<TransformComponent>(self.root)
            .unwrap();
        root.transform.translation = self.root_local.translation;
        root.transform.recompute_model();
        root.transform.matrix_world = self.root_world;
        emit.push_intent_now(
            self.root,
            IntentValue::UpdateTransformWorld {
                component_id: self.root,
            },
        );
        emit_local(emit, self.neck, self.neck_local);
        if let Some((mount, local)) = self.mount_local {
            emit_local(emit, mount, local);
        }
    }
}

#[derive(Debug, Default)]
pub struct HeadNeckBodyConstraintSystem {
    avatars: HashSet<ComponentId>,
    statuses: HashMap<ComponentId, HeadNeckBodyConstraintStatus>,
}

impl HeadNeckBodyConstraintSystem {
    pub fn register(&mut self, id: ComponentId) {
        self.avatars.insert(id);
    }
    pub fn remove(&mut self, id: ComponentId) {
        self.avatars.remove(&id);
        self.statuses.remove(&id);
    }
    pub fn status(&self, id: ComponentId) -> HeadNeckBodyConstraintStatus {
        self.statuses
            .get(&id)
            .copied()
            .unwrap_or(HeadNeckBodyConstraintStatus::Pending)
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        attachments: &AttachmentSystem,
    ) {
        for &id in &self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            let mut ancestor = Some(id);
            let mut mounted = false;
            while let Some(node) = ancestor {
                if attachments.is_movement_root_mounted(world, node) {
                    mounted = true;
                    break;
                }
                ancestor = world.parent_of(node);
            }
            let result = if !avatar.head_neck_body_enabled || !avatar.neck_pin_enabled {
                Err(HeadNeckBodyConstraintStatus::Disabled)
            } else if mounted {
                Err(HeadNeckBodyConstraintStatus::Mounted)
            } else if let Some(constraint) = &avatar.head_neck_body_constraint {
                constraint.candidate(world, avatar)
            } else {
                Err(HeadNeckBodyConstraintStatus::UnsupportedRig(
                    avatar
                        .head_neck_body_bind_error
                        .unwrap_or("waiting for head/neck binding"),
                ))
            };
            match result {
                Ok(candidate) => {
                    candidate.apply(world, emit);
                    world
                        .get_component_by_id_as_mut::<AvatarControlComponent>(id)
                        .unwrap()
                        .head_neck_body_constraint
                        .as_mut()
                        .unwrap()
                        .active = true;
                    self.statuses
                        .insert(id, HeadNeckBodyConstraintStatus::Ready);
                }
                Err(status) => {
                    if matches!(
                        status,
                        HeadNeckBodyConstraintStatus::Pending
                            | HeadNeckBodyConstraintStatus::InvalidPose
                    ) {
                        self.statuses.insert(id, status);
                        continue;
                    }
                    // Borrow the runtime out temporarily to restore its rest pose.
                    let mut constraint = world
                        .get_component_by_id_as_mut::<AvatarControlComponent>(id)
                        .unwrap()
                        .head_neck_body_constraint
                        .take();
                    let mut fallback_head = None;
                    if let Some(constraint) = &mut constraint {
                        let avatar = world
                            .get_component_by_id_as::<AvatarControlComponent>(id)
                            .unwrap();
                        fallback_head = constraint.restore(world, avatar, emit);
                    }
                    world
                        .get_component_by_id_as_mut::<AvatarControlComponent>(id)
                        .unwrap()
                        .head_neck_body_constraint = constraint;
                    if self.statuses.insert(id, status) != Some(status)
                        && let HeadNeckBodyConstraintStatus::UnsupportedRig(reason) = status
                        && world
                            .get_component_by_id_as::<AvatarControlComponent>(id)
                            .unwrap()
                            .head_mount
                            .is_some()
                    {
                        eprintln!("[HeadNeckBodyConstraint] {id:?}: fallback: {reason}");
                    }
                    if !matches!(
                        status,
                        HeadNeckBodyConstraintStatus::Pending
                            | HeadNeckBodyConstraintStatus::InvalidPose
                    ) {
                        super::head_pose_body_xz_follow::tick_one(id, world, emit, fallback_head);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
