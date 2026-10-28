//! The only lower-body scheduler and pose writer. Per-avatar state lives here.
use super::leg::solve_leg;
use super::rig::*;
use super::{FootGoal, FootPhase, FootState, LowerBodyStatus, crouch, support::SupportQuery};
use super::{WalkingDiagnostics, walking::WalkingState};
use crate::engine::ecs::component::{AvatarControlComponent, InputComponent, InputXRComponent};
use crate::engine::ecs::system::collidable_bvh::CollidableBvh;
use crate::engine::ecs::system::static_contact_system::StaticContactSystem;
use crate::engine::ecs::system::{AttachmentSystem, HumanoidBoneMapSystem};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::utils::math::{mat4_mul, quat_rotate_vec3, vec3_len, vec3_sub};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct LowerBodyIkSystem {
    pub(super) avatars: HashMap<ComponentId, LowerBodyInstance>,
    traversal: Vec<usize>,
    candidates: Vec<usize>,
}
#[derive(Debug, Default)]
pub(super) struct LowerBodyInstance {
    pub(super) rig: Option<LowerBodyRig>,
    binding_key: Option<(ComponentId, u64)>,
    bind_error: Option<&'static str>,
    status: LowerBodyStatus,
    feet: Option<[FootState; 2]>,
    walking: WalkingState,
    sole_offsets: Option<[[f32; 3]; 2]>,
}
impl LowerBodyIkSystem {
    pub fn register(&mut self, id: ComponentId) {
        self.avatars.entry(id).or_default();
    }
    pub fn remove(&mut self, id: ComponentId) {
        self.avatars.remove(&id);
    }
    pub fn status(&self, id: ComponentId) -> LowerBodyStatus {
        self.avatars.get(&id).map(|a| a.status).unwrap_or_default()
    }
    pub fn feet(&self, id: ComponentId) -> Option<&[FootState; 2]> {
        self.avatars.get(&id)?.feet.as_ref()
    }
    pub fn walking(&self, id: ComponentId) -> Option<WalkingDiagnostics> {
        Some(self.avatars.get(&id)?.walking.diagnostics())
    }
    /// Capture resolved leg bindings before AVC's queued head splices invalidate
    /// the map report. Calibration still belongs to this system, never AVC.
    pub fn prepare_bindings(
        &mut self,
        world: &World,
        maps: &HumanoidBoneMapSystem,
        emit: &mut dyn SignalEmitter,
    ) {
        for (&id, instance) in &mut self.avatars {
            if let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) {
                instance.sync_binding(world, avatar, maps, emit);
            }
        }
    }
    pub fn tick(
        &mut self,
        world: &World,
        emit: &mut dyn SignalEmitter,
        contacts: &mut StaticContactSystem,
        attachments: &AttachmentSystem,
        maps: &HumanoidBoneMapSystem,
        dt_sec: f32,
    ) {
        if self.avatars.is_empty() {
            return;
        }
        let index = contacts.support_index(world);
        for (&id, instance) in &mut self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            instance.sync_binding(world, avatar, maps, emit);
            if instance.sole_offsets != avatar.crouch_sole_offsets {
                instance.walking.reset();
                instance.sole_offsets = avatar.crouch_sole_offsets;
            }
            let mut node = Some(id);
            let mut mounted = false;
            let mut tracking = None;
            while let Some(current) = node {
                mounted |= attachments.is_movement_root_mounted(world, current);
                if tracking.is_none() {
                    if let Some(input) = world.get_component_by_id_as::<InputXRComponent>(current) {
                        tracking = Some(input.enabled && input.pose_valid);
                    } else if let Some(input) =
                        world.get_component_by_id_as::<InputComponent>(current)
                    {
                        tracking = Some(input.enabled);
                    }
                }
                node = world.parent_of(current);
            }
            let gate = if !avatar.crouch_enabled && !avatar.walking_enabled {
                Some(LowerBodyStatus::Disabled)
            } else if mounted {
                Some(LowerBodyStatus::Mounted)
            } else if !tracking.unwrap_or(true) {
                Some(LowerBodyStatus::Pending)
            } else {
                None
            };
            let result = match (&mut instance.rig, gate) {
                (_, Some(status)) => Err(status),
                (Some(rig), None) => solve_frame(
                    rig,
                    &mut instance.walking,
                    world,
                    avatar,
                    id,
                    index,
                    &mut self.traversal,
                    &mut self.candidates,
                    emit,
                    dt_sec,
                ),
                (None, None) => Err(instance
                    .bind_error
                    .map(LowerBodyStatus::UnsupportedRig)
                    .unwrap_or_default()),
            };
            instance.status = match result {
                Ok(feet) => {
                    let status = if feet.iter().all(|f| f.support.is_none()) {
                        LowerBodyStatus::NoSupport
                    } else if feet
                        .iter()
                        .any(|f| f.residual > 0.01 || f.target_error > 0.01)
                    {
                        LowerBodyStatus::Limited
                    } else {
                        LowerBodyStatus::Ready
                    };
                    instance.feet = Some(feet);
                    status
                }
                Err(status) => {
                    if let Some(rig) = &mut instance.rig {
                        rig.restore(world, emit);
                    }
                    instance.walking.reset();
                    instance.feet = None;
                    status
                }
            };
        }
    }
}
impl LowerBodyInstance {
    fn sync_binding(
        &mut self,
        world: &World,
        avatar: &AvatarControlComponent,
        maps: &HumanoidBoneMapSystem,
        emit: &mut dyn SignalEmitter,
    ) {
        let Some(root) = avatar.model_root_id else {
            return;
        };
        let Some(report) = avatar.humanoid_map_gltf.and_then(|id| maps.report(id)) else {
            return;
        };
        let key = (root, report.generation);
        if self.binding_key == Some(key) {
            return;
        }
        self.binding_key = Some(key);
        if let Some(rig) = &mut self.rig {
            if rig.root == root && rig.matches_map(report) {
                rig.generation = report.generation;
                return;
            }
            rig.restore(world, emit);
        }
        self.rig = None;
        self.walking.reset();
        match LowerBodyRig::bind(world, root, report) {
            Ok(rig) => {
                self.rig = Some(rig);
                self.bind_error = None;
            }
            Err(reason) => self.bind_error = Some(reason),
        }
    }
}

fn solve_frame(
    rig: &mut LowerBodyRig,
    walking: &mut WalkingState,
    world: &World,
    avatar: &AvatarControlComponent,
    id: ComponentId,
    index: &CollidableBvh,
    stack: &mut Vec<usize>,
    candidates: &mut Vec<usize>,
    emit: &mut dyn SignalEmitter,
    dt_sec: f32,
) -> Result<[FootState; 2], LowerBodyStatus> {
    use LowerBodyStatus::UnsupportedRig;
    if avatar.model_root_id != Some(rig.root)
        || avatar.humanoid_map_generation != rig.generation
        || rig
            .ancestors
            .iter()
            .any(|&(node, parent)| world.parent_of(node) != parent)
    {
        return Err(UnsupportedRig("leg binding changed"));
    }
    let offsets = avatar
        .crouch_sole_offsets
        .ok_or(LowerBodyStatus::NeedsSoleCalibration)?;
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
    if avatar.walking_enabled
        && (!avatar.walking_stride.is_finite()
            || !(0.05..=1.0).contains(&avatar.walking_stride)
            || !avatar.walking_step_duration.is_finite()
            || !(0.08..=1.0).contains(&avatar.walking_step_duration)
            || !avatar.walking_step_height.is_finite()
            || !(0.0..=0.3).contains(&avatar.walking_step_height))
    {
        return Err(UnsupportedRig("invalid walking configuration"));
    }
    let body = uniform(matrix(world, rig.root).map_err(UnsupportedRig)?).map_err(UnsupportedRig)?;
    let pole = quat_rotate_vec3(body.rotation_quat_xyzw, avatar.crouch_knee_pole);
    let mut bases = [[[0.0; 4]; 4]; 2];
    let mut soles = [[0.0; 3]; 2];
    let mut rotations = [[0.0, 0.0, 0.0, 1.0]; 2];
    for (i, leg) in rig.legs.iter().enumerate() {
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
        rotations[i] = uniform(foot).map_err(UnsupportedRig)?.rotation_quat_xyzw;
        soles[i] = point(foot, offsets[i]);
        if !soles[i].into_iter().all(f32::is_finite) {
            return Err(UnsupportedRig("nonfinite sole landmark"));
        }
    }
    let padding = if avatar.walking_enabled {
        avatar.walking_stride * 3.0 * body.scale[0]
    } else {
        0.0
    };
    let ground = SupportQuery::new(
        world,
        index,
        avatar,
        id,
        soles,
        body.scale[0],
        padding,
        stack,
        candidates,
    )?;
    let rest = std::array::from_fn::<_, 2, _>(|i| {
        crouch::evaluate(
            FootGoal {
                sole: soles[i],
                rotation: rotations[i],
            },
            ground.at(soles[i]),
            avatar.crouch_enabled,
        )
    });
    let gait = if avatar.walking_enabled {
        walking.advance(
            rest,
            ground.center(),
            body.scale[0],
            avatar,
            dt_sec,
            &ground,
        )
    } else {
        walking.reset();
        None
    };
    let goals = std::array::from_fn::<_, 2, _>(|i| gait.map(|g| g[i].foot).unwrap_or(rest[i].foot));
    let supports = std::array::from_fn::<_, 2, _>(|i| {
        gait.map(|g| Some(g[i].support)).unwrap_or(rest[i].support)
    });
    let mut results = [None, None];
    // Validate and prepare both outputs before emitting either leg's pose.
    for i in 0..2 {
        let goal = goals[i];
        let solve =
            if gait.is_some() || vec3_len(vec3_sub(goal.sole, soles[i])) > 0.0001 * body.scale[0] {
                Some(
                    solve_leg(
                        &rig.legs[i],
                        bases[i],
                        offsets[i],
                        goal,
                        pole,
                        avatar.crouch_knee_limit,
                    )
                    .map_err(UnsupportedRig)?,
                )
            } else {
                None
            };
        results[i] = Some(solve);
    }
    let states = std::array::from_fn(|i| {
        let leg = &mut rig.legs[i];
        let support = supports[i];
        let penetration = rest[i].penetration;
        let solution = results[i].take().unwrap();
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
        FootState {
            support: support.map(|s| s.zone),
            support_height: support.map(|s| s.height),
            base_sole: soles[i],
            solved_sole,
            penetration,
            knee_flexion: flexion,
            target_sole: goals[i].sole,
            target_rotation: goals[i].rotation,
            target_error: vec3_len(vec3_sub(goals[i].sole, solved_sole)),
            phase: gait.map(|g| g[i].phase).unwrap_or(FootPhase::Rest),
            residual: support
                .map(|s| (s.height - solved_sole[1]).max(0.0))
                .unwrap_or(0.0),
        }
    });
    Ok(states)
}
