//! Spine rig calibration: mapped hips-to-head chain, rest poses, and the
//! root-frame geometry the body solver uses to pitch the torso. The upper
//! chest is optional: MMD rigs go hips, spine (上半身), chest (上半身2), neck.
use crate::engine::ecs::component::{HumanoidSlot, TransformComponent};
use crate::engine::ecs::system::HumanoidBoneMapReport;
use crate::engine::ecs::system::ik::rest_pose::{read_bone_rest_pose, rest_model_relative_to};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformMatrix, TransformTrs};
use crate::utils::math::{quat_conjugate, quat_from_axis_angle, quat_mul, vec3_len, vec3_sub};

/// Pitch shares: the pelvis takes half, the spine bones above it split the rest.
pub(crate) const PELVIS_SHARE: f32 = 0.5;
/// Torso bones: hips, spine, chest and an optional upper chest.
const MAX_BONES: usize = 4;

#[derive(Debug, Clone)]
pub(crate) struct SpineRig {
    pub(crate) root: ComponentId,
    /// Hips, spine, chest, then upper chest when mapped; `count` are in use.
    pub(crate) bones: [ComponentId; MAX_BONES],
    pub(crate) count: usize,
    rest: [TransformTrs; MAX_BONES],
    /// Each bone's rest rotation in the model-root frame.
    rest_in_root: [[f32; 4]; MAX_BONES],
    /// Root-frame rest offsets between consecutive torso bones, then last
    /// torso bone→neck and neck→head; `count + 1` are in use.
    segments: [[f32; 3]; MAX_BONES + 1],
    /// Root-frame rest offset from the hips to the midpoint of the hip joints.
    pub(crate) hips_to_hip_joints: [f32; 3],
    pub(crate) thigh_length: f32,
    /// Standing head height above the model root, from the rest pose.
    pub(crate) head_rest_height: f32,
    /// Root-frame axis that pitches the torso forward (the avatar's left).
    pub(crate) pitch_axis: [f32; 3],
    pub(crate) generation: u64,
    active: bool,
    body_mount: Option<ComponentId>,
}

fn rest_in(
    world: &World,
    root: ComponentId,
    bone: ComponentId,
) -> Result<TransformTrs, &'static str> {
    let matrix =
        rest_model_relative_to(world, root, bone).ok_or("spine bone outside model root")?;
    TransformTrs::from_matrix(matrix).map_err(|_| "invalid spine rest pose")
}

fn position(matrix: TransformMatrix) -> [f32; 3] {
    [matrix[3][0], matrix[3][1], matrix[3][2]]
}

impl SpineRig {
    pub(crate) fn bind(
        world: &World,
        root: ComponentId,
        report: &HumanoidBoneMapReport,
    ) -> Result<Self, &'static str> {
        let id = |slot| {
            report
                .target(slot)
                .map(|t| t.component)
                .ok_or("incomplete humanoid spine map")
        };
        let chest = id(HumanoidSlot::Chest)?;
        let upper_chest = report.target(HumanoidSlot::UpperChest).map(|t| t.component);
        // Unused trailing slots repeat the chest and are never read.
        let bones = [
            id(HumanoidSlot::Hips)?,
            id(HumanoidSlot::Spine)?,
            chest,
            upper_chest.unwrap_or(chest),
        ];
        let count = if upper_chest.is_some() { 4 } else { 3 };
        let neck = id(HumanoidSlot::Neck)?;
        let head = id(HumanoidSlot::Head)?;
        let mut chain = bones[..count].to_vec();
        chain.push(neck);
        for pair in chain.windows(2) {
            if world.parent_of(pair[1]) != Some(pair[0]) {
                return Err(
                    "spine joints require a direct hips/spine/chest/(upper chest)/neck chain",
                );
            }
        }
        let rest = bones.map(|bone| {
            let (t, r, s) = read_bone_rest_pose(world, bone);
            TransformTrs::new(t, r, s)
        });
        let mut rest_in_root = [[0.0, 0.0, 0.0, 1.0]; MAX_BONES];
        // Torso bones, then neck and head.
        let mut points = [[0.0; 3]; MAX_BONES + 2];
        for (i, &bone) in bones[..count].iter().enumerate() {
            let trs = rest_in(world, root, bone)?;
            rest_in_root[i] = trs.rotation_quat_xyzw;
            points[i] = trs.translation;
        }
        // The head is spliced under its mount at runtime; its rest position
        // relative to the neck still comes from the rest pose.
        points[count] = rest_in(world, root, neck)?.translation;
        let neck_to_head =
            rest_model_relative_to(world, neck, head).ok_or("head is not below mapped neck")?;
        let neck_matrix = rest_model_relative_to(world, root, neck).ok_or("neck outside root")?;
        points[count + 1] = crate::utils::math::mat4_mul_vec4(
            neck_matrix,
            [
                neck_to_head[3][0],
                neck_to_head[3][1],
                neck_to_head[3][2],
                1.0,
            ],
        )[..3]
            .try_into()
            .unwrap();
        let mut segments = [[0.0; 3]; MAX_BONES + 1];
        for i in 0..=count {
            segments[i] = vec3_sub(points[i + 1], points[i]);
        }
        if segments[..=count].iter().any(|s| vec3_len(*s) < 1e-5) {
            return Err("zero spine segment");
        }
        let left_hip = position(
            rest_model_relative_to(world, root, id(HumanoidSlot::LeftUpperLeg)?)
                .ok_or("hip joint outside root")?,
        );
        let right_hip = position(
            rest_model_relative_to(world, root, id(HumanoidSlot::RightUpperLeg)?)
                .ok_or("hip joint outside root")?,
        );
        let left_knee = position(
            rest_model_relative_to(world, root, id(HumanoidSlot::LeftLowerLeg)?)
                .ok_or("knee outside root")?,
        );
        let mid_hips = std::array::from_fn(|i| (left_hip[i] + right_hip[i]) / 2.0);
        let left = [left_hip[0] - right_hip[0], 0.0, left_hip[2] - right_hip[2]];
        let width = vec3_len(left);
        if width < 1e-4 {
            return Err("hip joints coincide");
        }
        Ok(Self {
            root,
            bones,
            count,
            rest,
            rest_in_root,
            segments,
            hips_to_hip_joints: vec3_sub(mid_hips, points[0]),
            thigh_length: vec3_len(vec3_sub(left_knee, left_hip)),
            head_rest_height: points[count + 1][1],
            pitch_axis: left.map(|v| v / width),
            generation: report.generation,
            active: false,
            body_mount: None,
        })
    }

    pub(crate) fn matches_map(&self, report: &HumanoidBoneMapReport) -> bool {
        let upper_chest = report.target(HumanoidSlot::UpperChest).map(|t| t.component);
        upper_chest.is_some() == (self.count == 4)
            && [
                HumanoidSlot::Hips,
                HumanoidSlot::Spine,
                HumanoidSlot::Chest,
                HumanoidSlot::UpperChest,
            ]
            .into_iter()
            .zip(self.bones)
            .take(self.count)
            .all(|(slot, id)| report.target(slot).is_some_and(|t| t.component == id))
    }

    /// Root-frame rest segments from the hips up to the head.
    pub(crate) fn segments(&self) -> &[[f32; 3]] {
        &self.segments[..=self.count]
    }

    /// Compensate for the chest joint's imported rest axes.
    /// Once attached, ordinary transform propagation follows the solved pose.
    pub(super) fn attach_body_mount(
        &mut self,
        world: &World,
        emit: &mut dyn SignalEmitter,
        mount: ComponentId,
    ) {
        let chest = self.bones[self.count - 1];
        let parent = world.parent_of(mount);
        if parent == Some(chest) && self.body_mount == Some(mount) {
            return;
        }
        if parent != Some(self.root) && parent != Some(chest) {
            return;
        }
        let mut ancestor = Some(chest);
        while let Some(id) = ancestor {
            if id == mount {
                return;
            }
            ancestor = world.parent_of(id);
        }
        let Ok(rest) = rest_in(world, self.root, chest) else {
            return;
        };
        if rest.scale.into_iter().any(|scale| scale <= 1e-5) {
            return;
        }
        let scale = rest.scale.map(|scale| scale.recip());
        let rotation = quat_conjugate(rest.rotation_quat_xyzw);
        let offset = crate::utils::math::quat_rotate_vec3(
            rotation,
            [0.0, -self.head_rest_height * 0.08, 0.0],
        );
        if parent != Some(chest) {
            emit.push_intent_now(
                chest,
                IntentValue::Attach {
                    parent: chest,
                    child: mount,
                },
            );
        }
        emit_pose(
            emit,
            mount,
            TransformTrs::new(
                std::array::from_fn(|i| offset[i] * scale[i]),
                rotation,
                scale,
            ),
        );
        self.body_mount = Some(mount);
    }

    /// Per-bone pitch for a total torso pitch, hips first; entries past
    /// `count` are zero.
    pub(crate) fn distribute(&self, pitch: f32) -> [f32; MAX_BONES] {
        let spine = pitch * (1.0 - PELVIS_SHARE) / (self.count - 1) as f32;
        std::array::from_fn(|i| match i {
            0 => pitch * PELVIS_SHARE,
            i if i < self.count => spine,
            _ => 0.0,
        })
    }

    /// Pitch the torso forward by `pitch`, distributed over the torso bones,
    /// each about the shared root-frame axis. Rotations about one axis compose
    /// additively down the chain.
    pub(crate) fn write(&mut self, world: &World, emit: &mut dyn SignalEmitter, pitch: f32) {
        let pitches = self.distribute(pitch);
        for i in 0..self.count {
            let id = self.bones[i];
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_none()
            {
                continue;
            }
            let axis = crate::utils::math::quat_rotate_vec3(
                quat_conjugate(self.rest_in_root[i]),
                self.pitch_axis,
            );
            let rotation = quat_mul(
                self.rest[i].rotation_quat_xyzw,
                quat_from_axis_angle(axis, pitches[i]),
            );
            emit_pose(
                emit,
                id,
                TransformTrs {
                    rotation_quat_xyzw: rotation,
                    ..self.rest[i]
                },
            );
        }
        self.active = true;
    }

    /// Return the torso bones to rest once after pitching ends.
    pub(crate) fn restore(&mut self, world: &World, emit: &mut dyn SignalEmitter) {
        if !self.active {
            return;
        }
        for i in 0..self.count {
            if world
                .get_component_by_id_as::<TransformComponent>(self.bones[i])
                .is_some()
            {
                emit_pose(emit, self.bones[i], self.rest[i]);
            }
        }
        self.active = false;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::system::ik::body::{KNEE_RADIUS, hip_joint_height, solve_pitch};

    const IDENTITY: TransformMatrix = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    /// An upright synthetic torso: `rises` are the segment heights from the
    /// hips up to the head, one more than the torso bone count.
    fn rig(rises: &[f32]) -> SpineRig {
        let mut world = World::default();
        let id = world.add_component(TransformComponent::new());
        let count = rises.len() - 1;
        let mut segments = [[0.0; 3]; MAX_BONES + 1];
        for (segment, rise) in segments.iter_mut().zip(rises) {
            *segment = [0.0, *rise, 0.0];
        }
        SpineRig {
            root: id,
            bones: [id; MAX_BONES],
            count,
            rest: [TransformTrs::new([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3]); MAX_BONES],
            rest_in_root: [[0.0, 0.0, 0.0, 1.0]; MAX_BONES],
            segments,
            hips_to_hip_joints: [0.0, -0.08, 0.0],
            thigh_length: 0.4,
            head_rest_height: rises.iter().sum(),
            pitch_axis: [1.0, 0.0, 0.0],
            generation: 0,
            active: false,
            body_mount: None,
        }
    }

    #[test]
    fn three_and_four_bone_torsos_share_out_the_whole_pitch() {
        for count in [3, 4] {
            let spine = rig(&vec![0.1; count + 1]);
            let shares = spine.distribute(1.0);
            assert!((shares.iter().sum::<f32>() - 1.0).abs() < 1e-6, "{count}: {shares:?}");
            assert_eq!(shares[0], PELVIS_SHARE);
            assert!(shares[count..].iter().all(|&share| share == 0.0));
        }
    }

    #[test]
    fn a_three_bone_torso_solves_hips_onto_the_kneeling_height() {
        // MMD-style: hips, spine, chest, then neck and head. The four-bone
        // twin splits the chest-to-neck segment at an upper chest.
        let three = rig(&[0.1, 0.15, 0.2, 0.1]);
        let four = rig(&[0.1, 0.15, 0.1, 0.1, 0.1]);
        let head_y = 0.7;
        assert_eq!(three.segments().len(), 4);
        assert!(
            (hip_joint_height(&three, IDENTITY, head_y, 0.0)
                - hip_joint_height(&four, IDENTITY, head_y, 0.0))
            .abs()
                < 1e-6
        );
        let target = three.thigh_length + KNEE_RADIUS;
        for spine in [&three, &four] {
            assert!(hip_joint_height(spine, IDENTITY, head_y, 0.0) < target);
            let pitch = solve_pitch(spine, IDENTITY, head_y, 0.0, 1.0);
            assert!(pitch > 0.1, "the torso pitches: {pitch}");
            let hips = hip_joint_height(spine, IDENTITY, head_y, pitch);
            assert!((hips - target).abs() < 1e-3, "count {}: {hips} vs {target}", spine.count);
        }
    }
}
