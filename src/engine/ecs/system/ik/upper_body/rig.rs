//! Spine rig calibration: mapped hips-to-head chain, rest poses, and the
//! root-frame geometry the body solver uses to pitch the torso.
use crate::engine::ecs::component::{HumanoidSlot, TransformComponent};
use crate::engine::ecs::system::HumanoidBoneMapReport;
use crate::engine::ecs::system::ik::rest_pose::{read_bone_rest_pose, rest_model_relative_to};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformMatrix, TransformTrs};
use crate::utils::math::{quat_conjugate, quat_from_axis_angle, quat_mul, vec3_len, vec3_sub};

/// Pitch shares: the pelvis takes half, the three spine segments split the rest.
pub(crate) const PELVIS_SHARE: f32 = 0.5;

#[derive(Debug, Clone)]
pub(crate) struct SpineRig {
    pub(crate) root: ComponentId,
    /// Hips, spine, chest, upper chest.
    pub(crate) bones: [ComponentId; 4],
    rest: [TransformTrs; 4],
    /// Each bone's rest rotation in the model-root frame.
    rest_in_root: [[f32; 4]; 4],
    /// Root-frame rest offsets: hips→spine, spine→chest, chest→upper chest,
    /// upper chest→neck, neck→head.
    pub(crate) segments: [[f32; 3]; 5],
    /// Root-frame rest offset from the hips to the midpoint of the hip joints.
    pub(crate) hips_to_hip_joints: [f32; 3],
    pub(crate) thigh_length: f32,
    /// Standing head height above the model root, from the rest pose.
    pub(crate) head_rest_height: f32,
    /// Root-frame axis that pitches the torso forward (the avatar's left).
    pub(crate) pitch_axis: [f32; 3],
    pub(crate) generation: u64,
    active: bool,
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
        let bones = [
            id(HumanoidSlot::Hips)?,
            id(HumanoidSlot::Spine)?,
            id(HumanoidSlot::Chest)?,
            id(HumanoidSlot::UpperChest)?,
        ];
        let neck = id(HumanoidSlot::Neck)?;
        let head = id(HumanoidSlot::Head)?;
        for pair in [bones[0], bones[1], bones[2], bones[3], neck].windows(2) {
            if world.parent_of(pair[1]) != Some(pair[0]) {
                return Err(
                    "spine joints require a direct hips/spine/chest/upper-chest/neck chain",
                );
            }
        }
        let rest = bones.map(|bone| {
            let (t, r, s) = read_bone_rest_pose(world, bone);
            TransformTrs::new(t, r, s)
        });
        let mut rest_in_root = [[0.0, 0.0, 0.0, 1.0]; 4];
        let mut points = [[0.0; 3]; 6];
        for (i, &bone) in bones.iter().enumerate() {
            let trs = rest_in(world, root, bone)?;
            rest_in_root[i] = trs.rotation_quat_xyzw;
            points[i] = trs.translation;
        }
        // The head is spliced under its mount at runtime; its rest position
        // relative to the neck still comes from the rest pose.
        points[4] = rest_in(world, root, neck)?.translation;
        let neck_to_head =
            rest_model_relative_to(world, neck, head).ok_or("head is not below mapped neck")?;
        let neck_matrix = rest_model_relative_to(world, root, neck).ok_or("neck outside root")?;
        points[5] = crate::utils::math::mat4_mul_vec4(
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
        let segments = std::array::from_fn(|i| vec3_sub(points[i + 1], points[i]));
        if segments.iter().any(|s| vec3_len(*s) < 1e-5) {
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
            rest,
            rest_in_root,
            segments,
            hips_to_hip_joints: vec3_sub(mid_hips, points[0]),
            thigh_length: vec3_len(vec3_sub(left_knee, left_hip)),
            head_rest_height: points[5][1],
            pitch_axis: left.map(|v| v / width),
            generation: report.generation,
            active: false,
        })
    }

    pub(crate) fn matches_map(&self, report: &HumanoidBoneMapReport) -> bool {
        [
            HumanoidSlot::Hips,
            HumanoidSlot::Spine,
            HumanoidSlot::Chest,
            HumanoidSlot::UpperChest,
        ]
        .into_iter()
        .zip(self.bones)
        .all(|(slot, id)| report.target(slot).is_some_and(|t| t.component == id))
    }

    /// Per-bone pitch for a total torso pitch: hips, spine, chest, upper chest.
    pub(crate) fn distribute(pitch: f32) -> [f32; 4] {
        let spine = pitch * (1.0 - PELVIS_SHARE) / 3.0;
        [pitch * PELVIS_SHARE, spine, spine, spine]
    }

    /// Pitch the torso forward by `pitch`, distributed over the four bones,
    /// each about the shared root-frame axis. Rotations about one axis compose
    /// additively down the chain.
    pub(crate) fn write(&mut self, world: &World, emit: &mut dyn SignalEmitter, pitch: f32) {
        let pitches = Self::distribute(pitch);
        for i in 0..4 {
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
        for i in 0..4 {
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
