//! Native lower-body pose policy. Reads shared collision support; writes only
//! thigh, knee and foot rotations. Tracking and contact response keep ownership.
use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    AvatarControlComponent, HumanoidSlot, InputXRComponent, TransformComponent, ZoneComponent,
};
use crate::engine::ecs::system::avatar_control_system::{
    read_bone_rest_pose, rest_model_relative_to,
};
use crate::engine::ecs::system::collidable_bvh::CollidableBvh;
use crate::engine::ecs::system::static_contact_system::StaticContactSystem;
use crate::engine::ecs::system::{AttachmentSystem, HumanoidBoneMapReport, collision_geometry};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformMatrix, TransformTrs};
use crate::utils::math::{
    mat4_mul, mat4_mul_vec4, quat_conjugate, quat_mul, quat_rotate_vec3, shortest_arc_quat,
    vec3_add, vec3_cross, vec3_dot, vec3_len, vec3_normalize, vec3_scale, vec3_sub,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrouchStatus {
    Pending,
    Disabled,
    Mounted,
    NeedsSoleCalibration,
    NoSupport,
    Ready,
    Limited,
    UnsupportedRig(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct FootCrouchState {
    pub support: Option<ComponentId>,
    pub support_height: Option<f32>,
    pub base_sole: [f32; 3],
    pub solved_sole: [f32; 3],
    pub penetration: f32,
    pub knee_flexion: f32,
    pub residual: f32,
}

#[derive(Debug, Clone)]
struct Leg {
    joints: [ComponentId; 3],
    parent: ComponentId,
    rest: [TransformTrs; 3],
    rest_flexion: f32,
    active: bool,
}

/// Rest calibration is per rig, independent of the user's startup head height.
#[derive(Debug, Clone)]
pub struct HumanoidCrouchConstraint {
    root: ComponentId,
    hips: ComponentId,
    legs: [Leg; 2],
    ancestors: Vec<(ComponentId, Option<ComponentId>)>,
    pub(crate) generation: u64,
}

fn uniform(matrix: TransformMatrix) -> Result<TransformTrs, &'static str> {
    let trs = TransformTrs::from_matrix(matrix).map_err(|_| "invalid/sheared leg frame")?;
    if trs.scale[0] <= 0.0
        || trs
            .scale
            .iter()
            .any(|s| (*s - trs.scale[0]).abs() > trs.scale[0] * 1e-4)
    {
        return Err("nonuniform leg scale");
    }
    Ok(trs)
}
fn matrix(world: &World, id: ComponentId) -> Result<TransformMatrix, &'static str> {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| t.transform.matrix_world)
        .ok_or("missing leg transform")
}
fn point(matrix: TransformMatrix, p: [f32; 3]) -> [f32; 3] {
    let p = mat4_mul_vec4(matrix, [p[0], p[1], p[2], 1.0]);
    [p[0], p[1], p[2]]
}
fn emit_pose(emit: &mut dyn SignalEmitter, id: ComponentId, pose: TransformTrs) {
    emit.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_id: id,
            translation: pose.translation,
            rotation_quat_xyzw: pose.rotation_quat_xyzw,
            scale: pose.scale,
        },
    );
}
fn below(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
    loop {
        if node == ancestor {
            return true;
        }
        let Some(parent) = world.parent_of(node) else {
            return false;
        };
        node = parent;
    }
}

impl HumanoidCrouchConstraint {
    pub(crate) fn bind(
        world: &World,
        root: ComponentId,
        report: &HumanoidBoneMapReport,
    ) -> Result<Self, &'static str> {
        if !report.lower_body_ready() {
            return Err("incomplete humanoid leg map");
        }
        let id = |slot| {
            report
                .target(slot)
                .map(|t| t.component)
                .ok_or("incomplete humanoid leg map")
        };
        let hips = id(HumanoidSlot::Hips)?;
        rest_model_relative_to(world, root, hips).ok_or("hips outside model root")?;
        let mut ancestors = Vec::new();
        let mut leg = |slots: [HumanoidSlot; 3]| -> Result<Leg, &'static str> {
            let joints = [id(slots[0])?, id(slots[1])?, id(slots[2])?];
            if world.parent_of(joints[1]) != Some(joints[0])
                || world.parent_of(joints[2]) != Some(joints[1])
            {
                return Err("leg joints require direct thigh/knee/foot hierarchy");
            }
            if !below(world, joints[0], hips) {
                return Err("thigh outside hips");
            }
            let parent = world.parent_of(joints[0]).ok_or("missing thigh parent")?;
            matrix(world, parent)?;
            let rest = joints.map(|joint| {
                let (t, r, s) = read_bone_rest_pose(world, joint);
                TransformTrs::new(t, r, s)
            });
            for pose in rest {
                uniform(pose.to_matrix().map_err(|_| "invalid leg rest pose")?)?;
            }
            if vec3_len(rest[1].translation) < 1e-4 || vec3_len(rest[2].translation) < 1e-4 {
                return Err("zero leg segment");
            }
            let mut node = joints[2];
            while node != root {
                let parent = world.parent_of(node);
                ancestors.push((node, parent));
                node = parent.ok_or("leg outside model root")?;
            }
            Ok(Leg {
                joints,
                parent,
                rest,
                rest_flexion: vec3_dot(
                    vec3_normalize(rest[1].translation),
                    quat_rotate_vec3(
                        rest[1].rotation_quat_xyzw,
                        vec3_normalize(rest[2].translation),
                    ),
                )
                .clamp(-1.0, 1.0)
                .acos(),
                active: false,
            })
        };
        let legs = [
            leg([
                HumanoidSlot::LeftUpperLeg,
                HumanoidSlot::LeftLowerLeg,
                HumanoidSlot::LeftFoot,
            ])?,
            leg([
                HumanoidSlot::RightUpperLeg,
                HumanoidSlot::RightLowerLeg,
                HumanoidSlot::RightFoot,
            ])?,
        ];
        if legs[0].joints.iter().any(|id| legs[1].joints.contains(id)) {
            return Err("leg maps share joints");
        }
        Ok(Self {
            root,
            hips,
            legs,
            ancestors,
            generation: report.generation,
        })
    }

    pub(crate) fn matches_map(&self, report: &HumanoidBoneMapReport) -> bool {
        let slots = [
            HumanoidSlot::LeftUpperLeg,
            HumanoidSlot::LeftLowerLeg,
            HumanoidSlot::LeftFoot,
            HumanoidSlot::RightUpperLeg,
            HumanoidSlot::RightLowerLeg,
            HumanoidSlot::RightFoot,
        ];
        report
            .target(HumanoidSlot::Hips)
            .is_some_and(|t| t.component == self.hips)
            && slots
                .into_iter()
                .zip(self.legs.iter().flat_map(|leg| leg.joints))
                .all(|(slot, id)| report.target(slot).is_some_and(|t| t.component == id))
    }

    pub(crate) fn restore(&mut self, world: &World, emit: &mut dyn SignalEmitter) {
        for leg in &mut self.legs {
            if leg.active {
                for (id, rest) in leg.joints.into_iter().zip(leg.rest) {
                    let same_parent = self
                        .ancestors
                        .iter()
                        .any(|&(joint, parent)| joint == id && world.parent_of(id) == parent);
                    if same_parent
                        && below(world, id, self.root)
                        && world
                            .get_component_by_id_as::<TransformComponent>(id)
                            .is_some()
                    {
                        emit_pose(emit, id, rest);
                    }
                }
                leg.active = false;
            }
        }
    }

    fn solve(
        &mut self,
        world: &World,
        avatar: &AvatarControlComponent,
        id: ComponentId,
        index: &CollidableBvh,
        stack: &mut Vec<usize>,
        candidates: &mut Vec<usize>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<[FootCrouchState; 2], CrouchStatus> {
        use CrouchStatus::UnsupportedRig;
        if avatar.model_root_id != Some(self.root)
            || avatar.humanoid_map_generation != self.generation
            || self
                .ancestors
                .iter()
                .any(|&(node, parent)| world.parent_of(node) != parent)
        {
            return Err(UnsupportedRig("leg binding changed"));
        }
        let offsets = avatar
            .crouch_sole_offsets
            .ok_or(CrouchStatus::NeedsSoleCalibration)?;
        if !avatar.crouch_knee_limit.is_finite()
            || !(0.0..std::f32::consts::PI).contains(&avatar.crouch_knee_limit)
            || !avatar.crouch_support_margin.is_finite()
            || avatar.crouch_support_margin <= 0.0
            || !offsets
                .iter()
                .flatten()
                .chain(avatar.crouch_knee_pole.iter())
                .all(|v| v.is_finite())
            || vec3_len(avatar.crouch_knee_pole) < 1e-5
            || !vec3_len(avatar.crouch_knee_pole).is_finite()
        {
            return Err(UnsupportedRig("invalid crouch calibration"));
        }
        if !avatar.capsule_ready || !avatar.collision_enabled {
            return Err(CrouchStatus::Pending);
        }
        let capsule = avatar.capsule_transform_id.ok_or(CrouchStatus::Pending)?;
        let collider = avatar.capsule_collidable_id.ok_or(CrouchStatus::Pending)?;
        let zone = world.parent_of(collider).ok_or(CrouchStatus::Pending)?;
        let shape = world
            .get_component_by_id_as::<ZoneComponent>(zone)
            .filter(|z| z.enabled)
            .ok_or(CrouchStatus::Pending)?
            .shape;
        let frame = matrix(world, capsule).map_err(UnsupportedRig)?;
        let (center, shape) = collision_geometry::axis_aligned_world_shape(shape, frame)
            .ok_or(UnsupportedRig("invalid capsule frame"))?;
        let (mut min, mut max) = collision_geometry::transformed_aabb(
            shape,
            crate::engine::transform::TransformTrs::new(center, [0.0, 0.0, 0.0, 1.0], [1.0; 3])
                .to_matrix()
                .unwrap(),
        );
        let bottom = min[1];
        let body =
            uniform(matrix(world, self.root).map_err(UnsupportedRig)?).map_err(UnsupportedRig)?;
        let pole = quat_rotate_vec3(body.rotation_quat_xyzw, avatar.crouch_knee_pole);
        let margin = avatar.crouch_support_margin * body.scale[0];
        if !margin.is_finite() {
            return Err(UnsupportedRig("nonfinite support margin"));
        }
        let mut bases = [[[0.0; 4]; 4]; 2];
        let mut soles = [[0.0; 3]; 2];
        for (i, leg) in self.legs.iter().enumerate() {
            let upper = mat4_mul(
                matrix(world, leg.parent).map_err(UnsupportedRig)?,
                leg.rest[0]
                    .to_matrix()
                    .map_err(|_| UnsupportedRig("invalid leg rest"))?,
            );
            uniform(upper).map_err(UnsupportedRig)?;
            bases[i] = upper;
            let foot = mat4_mul(
                mat4_mul(upper, leg.rest[1].to_matrix().unwrap()),
                leg.rest[2].to_matrix().unwrap(),
            );
            uniform(foot).map_err(UnsupportedRig)?;
            soles[i] = point(foot, offsets[i]);
            if !soles[i].into_iter().all(f32::is_finite) {
                return Err(UnsupportedRig("nonfinite sole landmark"));
            }
            for axis in 0..3 {
                min[axis] = min[axis].min(soles[i][axis]);
                max[axis] = max[axis].max(soles[i][axis]);
            }
        }
        for axis in 0..3 {
            min[axis] -= margin;
            max[axis] += margin;
        }
        index.query(min, max, stack, candidates);
        let owner = world.parent_of(id).ok_or(CrouchStatus::Pending)?;
        let mut results = [None, None];
        // Validate and prepare both outputs before emitting either leg's pose.
        for i in 0..2 {
            let mut support: Option<(ComponentId, f32)> = None;
            for &candidate in candidates.iter() {
                let surface = index.surface(candidate);
                if below(world, surface.zone, owner) || below(world, surface.frame_id, owner) {
                    continue;
                }
                let Some(height) = surface.flat_support(soles[i], center, bottom, margin) else {
                    continue;
                };
                if support
                    .is_none_or(|(_, previous)| (height - bottom).abs() < (previous - bottom).abs())
                {
                    support = Some((surface.zone, height));
                }
            }
            let penetration = support
                .map(|(_, height)| (height - soles[i][1]).max(0.0))
                .unwrap_or(0.0);
            let solve = if penetration > 0.0001 * body.scale[0] {
                Some(
                    solve_leg(
                        &self.legs[i],
                        bases[i],
                        offsets[i],
                        penetration,
                        pole,
                        avatar.crouch_knee_limit,
                    )
                    .map_err(UnsupportedRig)?,
                )
            } else {
                None
            };
            results[i] = Some((support, penetration, solve));
        }
        let states = std::array::from_fn(|i| {
            let leg = &mut self.legs[i];
            let (support, penetration, solution) = results[i].take().unwrap();
            let (solved_sole, flexion) = if let Some(solution) = solution {
                for (joint, local) in leg.joints.into_iter().zip(solution.local) {
                    emit_pose(emit, joint, local);
                }
                leg.active = true;
                (solution.sole, solution.flexion)
            } else {
                // Supported standing legs also own their base pose; animation
                // must not move the real soles away from the predicted rest FK.
                if leg.active || support.is_some() {
                    for (joint, rest) in leg.joints.into_iter().zip(leg.rest) {
                        emit_pose(emit, joint, rest);
                    }
                }
                leg.active = support.is_some();
                (soles[i], leg.rest_flexion)
            };
            FootCrouchState {
                support: support.map(|s| s.0),
                support_height: support.map(|s| s.1),
                base_sole: soles[i],
                solved_sole,
                penetration,
                knee_flexion: flexion,
                residual: support
                    .map(|(_, y)| (y - solved_sole[1]).max(0.0))
                    .unwrap_or(0.0),
            }
        });
        Ok(states)
    }
}

struct LegSolution {
    local: [TransformTrs; 3],
    sole: [f32; 3],
    flexion: f32,
}
fn solve_leg(
    leg: &Leg,
    upper: TransformMatrix,
    sole_offset: [f32; 3],
    penetration: f32,
    pole: [f32; 3],
    max_flexion: f32,
) -> Result<LegSolution, &'static str> {
    let mid = mat4_mul(upper, leg.rest[1].to_matrix().unwrap());
    let foot = mat4_mul(mid, leg.rest[2].to_matrix().unwrap());
    let upper_trs = uniform(upper)?;
    let mid_trs = uniform(mid)?;
    let foot_trs = uniform(foot)?;
    let root = upper_trs.translation;
    let old_mid = mid_trs.translation;
    let old_end = foot_trs.translation;
    let target = vec3_add(old_end, [0.0, penetration, 0.0]);
    let a = vec3_len(vec3_sub(old_mid, root));
    let b = vec3_len(vec3_sub(old_end, old_mid));
    if !a.is_finite() || !b.is_finite() || a < 1e-6 || b < 1e-6 || !(a + b).is_finite() {
        return Err("degenerate world-space leg segment");
    }
    let raw = vec3_sub(target, root);
    // Never bend the ankle above the hip on an unsupported very deep crouch.
    // Clamp to the last reachable downward target and expose its residual.
    let target_vector = [raw[0], raw[1].min(-1e-5 * (a + b)), raw[2]];
    let raw_reach = vec3_len(target_vector);
    if !raw_reach.is_finite() {
        return Err("nonfinite ankle target");
    }
    let min_reach = (a * a + b * b + 2.0 * a * b * max_flexion.cos())
        .max(0.0)
        .sqrt()
        .max((a - b).abs() + 1e-6 * (a + b));
    let reach = raw_reach.clamp(min_reach.min(a + b), a + b);
    let horizontal_squared = raw[0] * raw[0] + raw[2] * raw[2];
    let end = if horizontal_squared <= reach * reach {
        // Keep the stance: excess depth is vertical residual, not sliding feet.
        [
            target[0],
            root[1] - (reach * reach - horizontal_squared).max(0.0).sqrt(),
            target[2],
        ]
    } else {
        vec3_add(root, vec3_scale(vec3_normalize(target_vector), reach))
    };
    let direction = vec3_normalize(vec3_sub(end, root));
    let mut normal = vec3_cross(direction, pole);
    if vec3_len(normal) < 1e-5 {
        normal = vec3_cross(direction, [1.0, 0.0, 0.0]);
    }
    if vec3_len(normal) < 1e-5 {
        normal = vec3_cross(direction, [0.0, 0.0, 1.0]);
    }
    let knee = super::two_bone::midpoint(root, direction, vec3_normalize(normal), a, b, reach);
    let upper_delta = shortest_arc_quat(
        vec3_normalize(vec3_sub(old_mid, root)),
        vec3_normalize(vec3_sub(knee, root)),
    );
    let upper_rotation = quat_mul(upper_delta, upper_trs.rotation_quat_xyzw);
    let lower_delta = shortest_arc_quat(
        quat_rotate_vec3(upper_delta, vec3_normalize(vec3_sub(old_end, old_mid))),
        vec3_normalize(vec3_sub(end, knee)),
    );
    let lower_rotation = quat_mul(
        lower_delta,
        quat_mul(upper_delta, mid_trs.rotation_quat_xyzw),
    );
    let parent_rotation = quat_mul(
        upper_trs.rotation_quat_xyzw,
        quat_conjugate(leg.rest[0].rotation_quat_xyzw),
    );
    let rotations = [
        quat_mul(quat_conjugate(parent_rotation), upper_rotation),
        quat_mul(quat_conjugate(upper_rotation), lower_rotation),
        quat_mul(quat_conjugate(lower_rotation), foot_trs.rotation_quat_xyzw),
    ];
    let local = std::array::from_fn(|i| TransformTrs {
        rotation_quat_xyzw: rotations[i],
        ..leg.rest[i]
    });
    if local.iter().any(|p| !p.is_finite()) {
        return Err("nonfinite leg solve");
    }
    let scaled_offset = sole_offset.map(|v| v * foot_trs.scale[0]);
    let sole = vec3_add(
        end,
        quat_rotate_vec3(foot_trs.rotation_quat_xyzw, scaled_offset),
    );
    let flexion = ((reach * reach - a * a - b * b) / (2.0 * a * b))
        .clamp(-1.0, 1.0)
        .acos();
    Ok(LegSolution {
        local,
        sole,
        flexion,
    })
}

#[derive(Debug, Default)]
pub struct HumanoidCrouchSystem {
    avatars: HashSet<ComponentId>,
    statuses: HashMap<ComponentId, CrouchStatus>,
    feet: HashMap<ComponentId, [FootCrouchState; 2]>,
    traversal: Vec<usize>,
    candidates: Vec<usize>,
}
impl HumanoidCrouchSystem {
    pub fn register(&mut self, id: ComponentId) {
        self.avatars.insert(id);
    }
    pub fn remove(&mut self, id: ComponentId) {
        self.avatars.remove(&id);
        self.statuses.remove(&id);
        self.feet.remove(&id);
    }
    pub fn status(&self, id: ComponentId) -> CrouchStatus {
        self.statuses
            .get(&id)
            .copied()
            .unwrap_or(CrouchStatus::Pending)
    }
    pub fn feet(&self, id: ComponentId) -> Option<&[FootCrouchState; 2]> {
        self.feet.get(&id)
    }
    pub fn tick(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        contacts: &mut StaticContactSystem,
        attachments: &AttachmentSystem,
    ) {
        if self.avatars.is_empty() {
            return;
        }
        let index = contacts.support_index(world);
        for &id in &self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            let mut node = Some(id);
            let mut mounted = false;
            let mut tracking = None;
            while let Some(current) = node {
                mounted |= attachments.is_movement_root_mounted(world, current);
                if tracking.is_none() {
                    if let Some(input) = world.get_component_by_id_as::<InputXRComponent>(current) {
                        tracking = Some(input.enabled && input.pose_valid);
                    }
                }
                node = world.parent_of(current);
            }
            let gate = if !avatar.crouch_enabled {
                Some(CrouchStatus::Disabled)
            } else if mounted {
                Some(CrouchStatus::Mounted)
            } else if !tracking.unwrap_or(false) {
                Some(CrouchStatus::Pending)
            } else {
                None
            };
            let Some(mut constraint) = world
                .get_component_by_id_as_mut::<AvatarControlComponent>(id)
                .and_then(|c| c.crouch_constraint.take())
            else {
                let reason = world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .and_then(|c| c.crouch_bind_error);
                self.statuses.insert(
                    id,
                    gate.unwrap_or_else(|| {
                        reason
                            .map(CrouchStatus::UnsupportedRig)
                            .unwrap_or(CrouchStatus::Pending)
                    }),
                );
                continue;
            };
            let result = if let Some(status) = gate {
                Err(status)
            } else {
                constraint.solve(
                    world,
                    world
                        .get_component_by_id_as::<AvatarControlComponent>(id)
                        .unwrap(),
                    id,
                    index,
                    &mut self.traversal,
                    &mut self.candidates,
                    emit,
                )
            };
            let status = match result {
                Ok(feet) => {
                    let status = if feet.iter().all(|f| f.support.is_none()) {
                        CrouchStatus::NoSupport
                    } else if feet.iter().any(|f| f.residual > 0.01) {
                        CrouchStatus::Limited
                    } else {
                        CrouchStatus::Ready
                    };
                    self.feet.insert(id, feet);
                    status
                }
                Err(status) => {
                    constraint.restore(world, emit);
                    self.feet.remove(&id);
                    status
                }
            };
            world
                .get_component_by_id_as_mut::<AvatarControlComponent>(id)
                .unwrap()
                .crouch_constraint = Some(constraint);
            if self.statuses.insert(id, status) != Some(status) {
                if matches!(
                    status,
                    CrouchStatus::UnsupportedRig(_) | CrouchStatus::NeedsSoleCalibration
                ) {
                    eprintln!("[HumanoidCrouch] {id:?}: {status:?}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
