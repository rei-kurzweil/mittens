use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use super::RetargetBasisStatus;
use super::hand_tracking::*;
use super::joint_basis_retargeting_system::{imported_rest_space_root, rest_model_from_root};
use super::{
    HumanoidBoneMapReport, HumanoidBoneMapSystem, JointBasisRetargetingSystem, TransformSystem,
};
use crate::engine::ecs::component::{
    BoneRestPoseComponent, GLTFComponent, HandTrackingComponent, HumanoidSlot, TransformComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::*;

type Rotation = [f32; 4];

#[derive(Debug)]
struct FingerBone {
    target: ComponentId,
    canonical_to_rest: Rotation,
    pose: RotationOwner,
}

#[derive(Debug)]
struct FingerBinding {
    finger: usize,
    bones: Vec<FingerBone>,
}

#[derive(Debug)]
struct HandBinding {
    hand: ComponentId,
    canonical_to_hand: Rotation,
    fingers: Vec<FingerBinding>,
}

#[derive(Debug)]
struct RigBinding {
    owner: ComponentId,
    generation: u64,
    hands: [Option<HandBinding>; 2],
    basis_generations: Vec<(ComponentId, Option<u64>, Option<RetargetBasisStatus>)>,
    diagnostics: Vec<String>,
    status: [HandTrackingStatus; 2],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HandTrackingStatus {
    pub provider: Option<HandTrackingProvider>,
    pub epoch: Option<u64>,
    pub data_source: Option<HandDataSource>,
    pub tracked_fingers: usize,
}

/// Tracking overrides the evaluated animation rotation while available. Underlying
/// animation writes are observed before the override; on loss we release toward
/// that pose (immutable rest when no other writer has supplied one).
#[derive(Debug)]
struct RotationOwner {
    underlying: Rotation,
    last_written: Option<Rotation>,
    release_from: Option<Rotation>,
    release_elapsed: f32,
    tracked: bool,
}

impl RotationOwner {
    fn new(rest: Rotation) -> Self {
        Self {
            underlying: rest,
            last_written: None,
            release_from: None,
            release_elapsed: 0.0,
            tracked: false,
        }
    }

    fn update(
        &mut self,
        current: Rotation,
        desired: Option<Rotation>,
        dt: f32,
        release: f32,
    ) -> Option<Rotation> {
        if self
            .last_written
            .is_none_or(|last| !same_rotation(last, current))
        {
            self.underlying = current;
        }
        let result = if let Some(desired) = desired {
            self.tracked = true;
            self.release_from = None;
            Some(desired)
        } else {
            if self.tracked {
                self.tracked = false;
                self.release_from = self.last_written;
                self.release_elapsed = 0.0;
            }
            self.release_from.map(|from| {
                self.release_elapsed += dt;
                let amount = if release <= 0.0 {
                    1.0
                } else {
                    (self.release_elapsed / release).clamp(0.0, 1.0)
                };
                if amount >= 1.0 {
                    self.release_from = None;
                }
                quat_nlerp(from, self.underlying, amount)
            })
        };
        self.last_written = result;
        result
    }
}

#[derive(Debug, Default)]
pub struct HandTrackingSystem {
    registrations: HashSet<ComponentId>,
    sources: HashMap<HandTrackingProvider, Arc<HandTrackingFrame>>,
    watermarks: HashMap<HandTrackingProvider, (u64, u64)>,
    rigs: HashMap<ComponentId, RigBinding>,
}

impl HandTrackingSystem {
    pub fn register(&mut self, component: ComponentId) {
        self.registrations.insert(component);
    }

    pub fn unregister(
        &mut self,
        world: &World,
        component: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) {
        self.registrations.remove(&component);
        if let Some(rig) = self.rigs.remove(&component) {
            retire(world, rig, emit);
        }
    }

    pub fn component_removed(&mut self, component: ComponentId) {
        // Keep the binding until tick can restore surviving bones through the queue.
        self.registrations.remove(&component);
    }

    /// Adapters publish whole immutable frames; never patch hands in an existing frame.
    /// Older/out-of-order deliveries within a stream cannot resurrect a stale pose.
    pub fn publish(&mut self, frame: Arc<HandTrackingFrame>) {
        if self
            .watermarks
            .get(&frame.provider)
            .is_some_and(|&(epoch, sequence)| {
                frame.epoch < epoch || (frame.epoch == epoch && frame.sequence <= sequence)
            })
        {
            return;
        }
        self.watermarks
            .insert(frame.provider, (frame.epoch, frame.sequence));
        self.sources.insert(frame.provider, frame);
    }

    pub fn set_openxr_frame(&mut self, frame: Option<Arc<HandTrackingFrame>>) {
        if let Some(frame) = frame {
            self.publish(frame);
        } else {
            self.sources.remove(&HandTrackingProvider::OpenXr);
        }
    }

    pub fn diagnostics(&self, component: ComponentId) -> &[String] {
        self.rigs
            .get(&component)
            .map(|rig| rig.diagnostics.as_slice())
            .unwrap_or(&[])
    }

    pub fn status(&self, component: ComponentId, side: usize) -> Option<HandTrackingStatus> {
        self.rigs.get(&component)?.status.get(side).copied()
    }

    pub fn selected_frame(
        &self,
        config: &HandTrackingComponent,
        side: usize,
        now: Instant,
    ) -> Option<&HandTrackingFrame> {
        if side >= 2 {
            return None;
        }
        [
            HandTrackingProvider::OpenXr,
            HandTrackingProvider::MediaPipe,
        ]
        .into_iter()
        .find_map(|provider| {
            if config
                .provider
                .is_some_and(|requested| requested != provider)
            {
                return None;
            }
            let frame = self.sources.get(&provider)?;
            let age = now.checked_duration_since(frame.received_at)?;
            if age.as_secs_f32() > config.timeout_seconds {
                return None;
            }
            if matches!(
                frame.space,
                HandCoordinateSpace::Calibrated { mirrored: true, .. }
            ) {
                return None;
            }
            let hand = frame.hands[side].as_ref()?;
            if !hand.has_articulation() || unit_quaternion(hand.canonical_orientation?).is_none() {
                return None;
            }
            Some(frame.as_ref())
        })
    }

    /// Run after arm IK has flushed, before skinning. Wrist channels have a separate
    /// owner; fingers consume the actual avatar hand frame without writing it.
    pub fn tick(
        &mut self,
        world: &mut World,
        maps: &mut HumanoidBoneMapSystem,
        bases: &JointBasisRetargetingSystem,
        emit: &mut dyn SignalEmitter,
        dt: f32,
    ) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let now = Instant::now();
        let mut ids: Vec<_> = self
            .registrations
            .iter()
            .chain(self.rigs.keys())
            .copied()
            .collect();
        ids.sort();
        ids.dedup();
        let owners: HashMap<_, _> = self
            .registrations
            .iter()
            .filter_map(|id| owning_gltf(world, *id).map(|owner| (*id, owner)))
            .collect();
        for id in ids {
            let config = world
                .get_component_by_id_as::<HandTrackingComponent>(id)
                .cloned();
            let owner = owners.get(&id).copied();
            let unique = owner
                .is_some_and(|owner| owners.values().filter(|other| **other == owner).count() == 1);
            if !self.registrations.contains(&id) || config.is_none() || !unique {
                if let Some(rig) = self.rigs.remove(&id) {
                    retire(world, rig, emit);
                }
                continue;
            }
            let config = config.unwrap();
            let owner = owner.unwrap();
            if world
                .get_component_by_id_as::<GLTFComponent>(owner)
                .is_none_or(|gltf| gltf.armature_joint_transforms.is_empty())
            {
                if let Some(rig) = self.rigs.remove(&id) {
                    retire(world, rig, emit);
                }
                continue;
            }
            let generation = maps
                .request_report(world, owner)
                .map(|report| report.generation);
            let changed = self.rigs.get(&id).is_some_and(|rig| {
                rig.owner != owner
                    || Some(rig.generation) != generation
                    || rig
                        .basis_generations
                        .iter()
                        .any(|(target, generation, status)| {
                            bases.basis_for(*target).map(|basis| basis.generation) != *generation
                                || bases.status_for(*target) != *status
                        })
            });
            if changed {
                retire(world, self.rigs.remove(&id).unwrap(), emit);
                // Restore before reading underlying rotations into a replacement binding.
                continue;
            }
            if !self.rigs.contains_key(&id) {
                let Some(report) = maps.report(owner) else {
                    continue;
                };
                let rig = bind_rig(world, report, bases);
                for diagnostic in &rig.diagnostics {
                    eprintln!("[HandTracking] {id:?}: {diagnostic}");
                }
                self.rigs.insert(id, rig);
            }
            let samples: [Option<TrackedHand>; 2] = std::array::from_fn(|side| {
                self.selected_frame(&config, side, now)
                    .and_then(|frame| frame.hands[side].clone())
            });
            let mut status: [HandTrackingStatus; 2] = std::array::from_fn(|side| {
                self.selected_frame(&config, side, now)
                    .map(|frame| HandTrackingStatus {
                        provider: Some(frame.provider),
                        epoch: Some(frame.epoch),
                        data_source: frame.hands[side].as_ref().map(|hand| hand.data_source),
                        tracked_fingers: 0,
                    })
                    .unwrap_or_default()
            });
            let rig = self.rigs.get_mut(&id).unwrap();
            let mut desired_worlds = HashMap::new();
            for (side, binding) in rig.hands.iter_mut().enumerate() {
                let Some(binding) = binding else {
                    continue;
                };
                let actual_hand = world_rotation(world, binding.hand);
                let hand_frame = actual_hand.map(|q| quat_mul(q, binding.canonical_to_hand));
                for finger in &mut binding.fingers {
                    let frames = samples[side]
                        .as_ref()
                        .and_then(|sample| finger_frames(sample, finger.finger));
                    if frames.is_some() && hand_frame.is_some() {
                        status[side].tracked_fingers += 1;
                    }
                    let source_hand_inverse = samples[side]
                        .as_ref()
                        .and_then(|sample| sample.canonical_orientation)
                        .and_then(unit_quaternion)
                        .map(quat_conjugate);
                    for (segment, bone) in finger.bones.iter_mut().enumerate() {
                        let Some(transform) =
                            world.get_component_by_id_as::<TransformComponent>(bone.target)
                        else {
                            continue;
                        };
                        let Some(current) = unit_quaternion(transform.transform.rotation) else {
                            continue;
                        };
                        let parent_world =
                            evaluated_parent_rotation(world, bone.target, &desired_worlds);
                        let desired = frames
                            .zip(hand_frame)
                            .zip(source_hand_inverse)
                            .zip(parent_world)
                            .map(|(((frames, hand), source_inverse), parent)| {
                                let segment_world =
                                    quat_mul(hand, quat_mul(source_inverse, frames[segment]));
                                quat_normalize(quat_mul(
                                    quat_conjugate(parent),
                                    quat_mul(segment_world, quat_conjugate(bone.canonical_to_rest)),
                                ))
                            });
                        if let Some(rotation) =
                            bone.pose
                                .update(current, desired, dt, config.release_seconds)
                        {
                            queue_rotation(world, bone.target, rotation, emit);
                            if let Some(parent) = parent_world {
                                desired_worlds.insert(bone.target, quat_mul(parent, rotation));
                            }
                        }
                    }
                }
            }
            if rig.status != status {
                if hand_tracking_debug_enabled() {
                    eprintln!(
                        "[HandTracking] {id:?} left={:?} right={:?}",
                        status[0], status[1]
                    );
                }
                rig.status = status;
            }
        }
    }
}

fn owning_gltf(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    while let Some(parent) = world.parent_of(id) {
        if world
            .get_component_by_id_as::<GLTFComponent>(parent)
            .is_some()
        {
            return Some(parent);
        }
        id = parent;
    }
    None
}

fn is_below(world: &World, mut id: ComponentId, ancestor: ComponentId) -> bool {
    while let Some(parent) = world.parent_of(id) {
        if parent == ancestor {
            return true;
        }
        id = parent;
    }
    false
}

fn finger_slots(side: usize) -> [[HumanoidSlot; 3]; 5] {
    use HumanoidSlot::*;
    if side == 0 {
        [
            [LeftThumbMetacarpal, LeftThumbProximal, LeftThumbDistal],
            [LeftIndexProximal, LeftIndexIntermediate, LeftIndexDistal],
            [LeftMiddleProximal, LeftMiddleIntermediate, LeftMiddleDistal],
            [LeftRingProximal, LeftRingIntermediate, LeftRingDistal],
            [LeftLittleProximal, LeftLittleIntermediate, LeftLittleDistal],
        ]
    } else {
        [
            [RightThumbMetacarpal, RightThumbProximal, RightThumbDistal],
            [RightIndexProximal, RightIndexIntermediate, RightIndexDistal],
            [
                RightMiddleProximal,
                RightMiddleIntermediate,
                RightMiddleDistal,
            ],
            [RightRingProximal, RightRingIntermediate, RightRingDistal],
            [
                RightLittleProximal,
                RightLittleIntermediate,
                RightLittleDistal,
            ],
        ]
    }
}

fn bind_rig(
    world: &World,
    report: &HumanoidBoneMapReport,
    bases: &JointBasisRetargetingSystem,
) -> RigBinding {
    let mut rig = RigBinding {
        owner: report.owning_gltf,
        generation: report.generation,
        hands: [None, None],
        basis_generations: Vec::new(),
        diagnostics: Vec::new(),
        status: [HandTrackingStatus::default(); 2],
    };
    let mut used = HashSet::new();
    for side in 0..2 {
        match bind_hand(world, report, bases, side, &mut used, &mut rig) {
            Ok(binding) => rig.hands[side] = Some(binding),
            Err(error) => rig.diagnostics.push(format!(
                "{} hand: {error}",
                if side == 0 { "left" } else { "right" }
            )),
        }
    }
    rig
}

fn bind_hand(
    world: &World,
    report: &HumanoidBoneMapReport,
    bases: &JointBasisRetargetingSystem,
    side: usize,
    used: &mut HashSet<ComponentId>,
    rig: &mut RigBinding,
) -> Result<HandBinding, String> {
    use HumanoidSlot::*;
    let target = |slot| {
        report
            .target(slot)
            .map(|target| target.component)
            .ok_or_else(|| format!("unresolved {slot}"))
    };
    let hand = target(if side == 0 { LeftHand } else { RightHand })?;
    let root = imported_rest_space_root(world, report.owning_gltf, hand);
    let model = |id| rest_model_from_root(world, root, id);
    let position = |id| -> Result<[f32; 3], String> {
        let m = model(id)?;
        Ok([m[3][0], m[3][1], m[3][2]])
    };
    let hand_rotation =
        unit_quaternion(mat_to_quat(model(hand)?)).ok_or("invalid hand rest rotation")?;
    let slots = finger_slots(side);
    let hand_basis = bases.basis_for(hand);
    let status = bases.status_for(hand);
    rig.basis_generations.push((
        hand,
        hand_basis.map(|basis| basis.generation),
        status.clone(),
    ));
    if status.is_some_and(|status| status != RetargetBasisStatus::Ready) {
        return Err("authored hand retarget basis is not ready".into());
    }
    let hand_frame = if let Some(basis) = hand_basis {
        quat_mul(hand_rotation, mat_to_quat(basis.canonical_to_target_rest))
    } else {
        frame_from_directions(
            vec3_sub(
                position(target(slots[2][2])?)?,
                position(target(slots[2][0])?)?,
            ),
            vec3_sub(
                position(target(slots[1][0])?)?,
                position(target(slots[4][0])?)?,
            ),
        )
        .ok_or("degenerate hand rest landmarks")?
    };
    let mut binding = HandBinding {
        hand,
        canonical_to_hand: quat_mul(quat_conjugate(hand_rotation), hand_frame),
        fingers: Vec::new(),
    };
    for (finger, slots) in slots.into_iter().enumerate() {
        let result = (|| -> Result<FingerBinding, String> {
            let ids = [target(slots[0])?, target(slots[1])?, target(slots[2])?];
            if !is_below(world, ids[0], hand)
                || !is_below(world, ids[1], ids[0])
                || !is_below(world, ids[2], ids[1])
            {
                return Err(
                    "finger chain must descend from its mapped hand in anatomical order".into(),
                );
            }
            if ids.iter().any(|id| used.contains(id)) {
                return Err("finger rotation target is mapped more than once".into());
            }
            let positions = [position(ids[0])?, position(ids[1])?, position(ids[2])?];
            let first = vec3_sub(positions[1], positions[0]);
            let second = vec3_sub(positions[2], positions[1]);
            // Explicit VRoid leaf policy: no imported tip, so the distal rest
            // direction continues the incoming rest segment. Tips drive no extra bone.
            let frames = segment_frames(hand_frame, [first, second, second])
                .ok_or("degenerate finger rest segments")?;
            let mut bones = Vec::new();
            for segment in 0..3 {
                let target = ids[segment];
                let rest = world
                    .children_of(target)
                    .iter()
                    .find_map(|id| world.get_component_by_id_as::<BoneRestPoseComponent>(*id))
                    .ok_or("missing immutable bone rest pose")?;
                let basis = bases.basis_for(target);
                let status = bases.status_for(target);
                rig.basis_generations.push((
                    target,
                    basis.map(|basis| basis.generation),
                    status.clone(),
                ));
                if status.is_some_and(|status| status != RetargetBasisStatus::Ready) {
                    return Err(format!(
                        "retarget basis for {} is not ready",
                        slots[segment]
                    ));
                }
                let bone_rotation = mat_to_quat(model(target)?);
                bones.push(FingerBone {
                    target,
                    canonical_to_rest: basis
                        .map(|basis| mat_to_quat(basis.canonical_to_target_rest))
                        .unwrap_or_else(|| {
                            quat_mul(quat_conjugate(bone_rotation), frames[segment])
                        }),
                    pose: RotationOwner::new(rest.rotation),
                });
            }
            used.extend(ids);
            Ok(FingerBinding { finger, bones })
        })();
        match result {
            Ok(finger) => binding.fingers.push(finger),
            Err(error) => rig.diagnostics.push(format!(
                "{} {}: {error}",
                if side == 0 { "left" } else { "right" },
                slots[0]
            )),
        }
    }
    Ok(binding)
}

fn frame_from_directions(forward: [f32; 3], up: [f32; 3]) -> Option<Rotation> {
    let forward = unit_direction(forward)?;
    let up = unit_direction(vec3_sub(up, vec3_scale(forward, vec3_dot(up, forward))))?;
    let z = vec3_scale(forward, -1.0);
    let right = vec3_cross(up, z);
    unit_quaternion(mat_to_quat([
        [right[0], right[1], right[2], 0.0],
        [up[0], up[1], up[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]))
}

fn unit_direction(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = vec3_len(v);
    (v.iter().all(|v| v.is_finite()) && length.is_finite() && length > 1e-6)
        .then(|| vec3_scale(v, 1.0 / length))
}

/// Transport palm width down the chain. This supplies stable roll through a fist
/// without projecting a fixed palm normal onto a segment that can point along it.
/// Near-antiparallel adjacent segments are invalid anatomy: release that finger.
fn segment_frames(hand: Rotation, directions: [[f32; 3]; 3]) -> Option<[Rotation; 3]> {
    let mut previous = unit_quaternion(hand)?;
    let mut frames = [[0.0; 4]; 3];
    for (index, direction) in directions.into_iter().enumerate() {
        let forward = quat_rotate_vec3(previous, [0.0, 0.0, -1.0]);
        let direction = unit_direction(direction)?;
        if vec3_dot(forward, direction) < -0.999 {
            return None;
        }
        previous = unit_quaternion(quat_mul(shortest_arc_quat(forward, direction), previous))?;
        frames[index] = previous;
    }
    Some(frames)
}

fn finger_frames(sample: &TrackedHand, finger: usize) -> Option<[Rotation; 3]> {
    let joints = HandJoint::FINGERS[finger];
    let p = [
        sample.position(joints[0])?,
        sample.position(joints[1])?,
        sample.position(joints[2])?,
        sample.position(joints[3])?,
    ];
    segment_frames(
        sample.canonical_orientation?,
        [
            vec3_sub(p[1], p[0]),
            vec3_sub(p[2], p[1]),
            vec3_sub(p[3], p[2]),
        ],
    )
}

fn world_rotation(world: &World, id: ComponentId) -> Option<Rotation> {
    unit_quaternion(mat_to_quat(TransformSystem::world_model(world, id)?))
}

fn evaluated_parent_rotation(
    world: &World,
    target: ComponentId,
    desired: &HashMap<ComponentId, Rotation>,
) -> Option<Rotation> {
    let parent = world.parent_of(target)?;
    let actual_parent = world_rotation(world, parent)?;
    let mut ancestor = Some(parent);
    while let Some(id) = ancestor {
        if let Some(rotation) = desired.get(&id) {
            // Carry the actual evaluated helper chain below the driven ancestor.
            return Some(quat_mul(
                *rotation,
                quat_mul(quat_conjugate(world_rotation(world, id)?), actual_parent),
            ));
        }
        ancestor = world.parent_of(id);
    }
    Some(actual_parent)
}

fn same_rotation(a: Rotation, b: Rotation) -> bool {
    a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>().abs() > 1.0 - 1e-6
}

fn queue_rotation(
    world: &World,
    target: ComponentId,
    rotation: Rotation,
    emit: &mut dyn SignalEmitter,
) {
    let Some(transform) = world.get_component_by_id_as::<TransformComponent>(target) else {
        return;
    };
    emit.push_intent_now(
        target,
        IntentValue::UpdateTransform {
            component_id: target,
            translation: transform.transform.translation,
            rotation_quat_xyzw: rotation,
            scale: transform.transform.scale,
        },
    );
}

fn retire(world: &World, rig: RigBinding, emit: &mut dyn SignalEmitter) {
    for hand in rig.hands.into_iter().flatten() {
        for finger in hand.fingers {
            for bone in finger.bones {
                let Some(last) = bone.pose.last_written else {
                    continue;
                };
                if world
                    .get_component_by_id_as::<TransformComponent>(bone.target)
                    .is_some_and(|t| same_rotation(t.transform.rotation, last))
                {
                    queue_rotation(world, bone.target, bone.pose.underlying, emit);
                }
            }
        }
    }
}

fn hand_tracking_debug_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_DEBUG_HAND_TRACKING").is_ok_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
    })
}

#[cfg(test)]
#[path = "hand_tracking_tests.rs"]
mod tests;
