//! Immutable rig calibration and ownership release, independent of posture policy.
use crate::engine::ecs::component::{HumanoidSlot, TransformComponent};
use crate::engine::ecs::system::HumanoidBoneMapReport;
use crate::engine::ecs::system::avatar_control_system::{
    read_bone_rest_pose, rest_model_relative_to,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformMatrix, TransformTrs};
use crate::utils::math::{mat4_mul_vec4, quat_rotate_vec3, vec3_dot, vec3_len, vec3_normalize};

#[derive(Debug, Clone)]
pub(super) struct Leg {
    pub(super) joints: [ComponentId; 3],
    pub(super) parent: ComponentId,
    pub(super) rest: [TransformTrs; 3],
    pub(super) rest_flexion: f32,
    pub(super) active: bool,
}

/// Rest calibration is per rig, independent of the user's startup head height.
#[derive(Debug, Clone)]
pub(super) struct LowerBodyRig {
    pub(super) root: ComponentId,
    pub(super) hips: ComponentId,
    pub(super) legs: [Leg; 2],
    pub(super) ancestors: Vec<(ComponentId, Option<ComponentId>)>,
    pub(crate) generation: u64,
}

pub(super) fn uniform(matrix: TransformMatrix) -> Result<TransformTrs, &'static str> {
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
pub(super) fn matrix(world: &World, id: ComponentId) -> Result<TransformMatrix, &'static str> {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| t.transform.matrix_world)
        .ok_or("missing leg transform")
}
pub(super) fn point(matrix: TransformMatrix, p: [f32; 3]) -> [f32; 3] {
    let p = mat4_mul_vec4(matrix, [p[0], p[1], p[2], 1.0]);
    [p[0], p[1], p[2]]
}
pub(super) fn emit_pose(emit: &mut dyn SignalEmitter, id: ComponentId, pose: TransformTrs) {
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
pub(super) fn below(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
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

impl LowerBodyRig {
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
}
