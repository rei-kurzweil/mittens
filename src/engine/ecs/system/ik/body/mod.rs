//! Body solver: the one place posture is decided. It picks the body mode for
//! each avatar and the torso pitch that mode needs. The upper body solver
//! writes the torso bones from that pitch; the lower body solver reads the
//! mode to choose standing or kneeling leg goals.
use super::lower_body::{LowerBodyIkSystem, LowerBodyStatus};
use super::upper_body::{SpineRig, UpperBodyIkSystem};
use crate::engine::ecs::component::{AvatarControlComponent, BodyPosture, TransformComponent};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{mat4_mul_vec4, quat_from_axis_angle, quat_rotate_vec3};
use std::collections::HashMap;

/// Vertical is everything an upright avatar does today. Horizontal is on hands
/// and knees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BodyMode {
    #[default]
    Vertical,
    Horizontal,
}

/// Auto mode enters horizontal below this fraction of standing head height...
const ENTER_HEAD_FRACTION: f32 = 0.42;
/// ...or below this one once crouching has run out.
const ENTER_LIMITED_HEAD_FRACTION: f32 = 0.55;
/// It returns to vertical above this fraction.
const EXIT_HEAD_FRACTION: f32 = 0.6;
/// Minimum time in a mode before auto may switch again.
const MIN_DWELL_SEC: f32 = 0.4;
/// Time to blend fully into or out of horizontal.
const BLEND_SEC: f32 = 0.35;
/// Knee contact clearance above the floor, in model units.
pub(crate) const KNEE_RADIUS: f32 = 0.06;
const MAX_PITCH: f32 = 1.75;
/// Body yaw deadband on hands and knees: the torso tracks the head closely.
const HORIZONTAL_YAW_THRESHOLD: f32 = 10.0 * std::f32::consts::PI / 180.0;

#[derive(Debug, Default)]
pub struct BodySolverSystem {
    avatars: HashMap<ComponentId, BodyInstance>,
}

#[derive(Debug, Default)]
struct BodyInstance {
    mode: BodyMode,
    /// 0 = fully vertical, 1 = fully horizontal.
    weight: f32,
    dwell: f32,
    support: Option<f32>,
    /// Last pitch solved in horizontal mode, faded out by `weight`.
    held_pitch: f32,
    pitch: f32,
}

impl BodySolverSystem {
    pub fn register(&mut self, id: ComponentId) {
        self.avatars.entry(id).or_default();
    }

    pub fn remove(&mut self, id: ComponentId) {
        self.avatars.remove(&id);
    }

    pub fn mode(&self, id: ComponentId) -> BodyMode {
        self.avatars.get(&id).map(|a| a.mode).unwrap_or_default()
    }

    /// Blend weight toward horizontal, in [0, 1].
    pub fn weight(&self, id: ComponentId) -> f32 {
        self.avatars.get(&id).map(|a| a.weight).unwrap_or(0.0)
    }

    /// Last known floor height under the avatar, kept across lost support.
    pub fn floor(&self, id: ComponentId) -> Option<f32> {
        self.avatars.get(&id).and_then(|a| a.support)
    }

    /// Forward torso pitch in radians, already blended.
    pub fn torso_pitch(&self, id: ComponentId) -> f32 {
        self.avatars.get(&id).map(|a| a.pitch).unwrap_or(0.0)
    }

    /// Tighten each avatar's body yaw-follow deadband as it blends into
    /// horizontal, from its authored `body_yaw_threshold` toward 10 degrees.
    pub fn apply_body_yaw(&self, world: &mut World) {
        use crate::engine::ecs::component::TransformOperatorComponent;
        use crate::engine::ecs::component::transform_operator::TransformQuatOp;
        for (&id, instance) in &self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            let Some(operator) = avatar.body_operator_id else {
                continue;
            };
            let authored = avatar.body_yaw_threshold;
            let wanted =
                authored + (HORIZONTAL_YAW_THRESHOLD.min(authored) - authored) * instance.weight;
            let Some(settings) = world
                .get_component_by_id_as_mut::<TransformOperatorComponent>(operator)
                .and_then(|o| o.fork_trs_settings_mut())
            else {
                continue;
            };
            for op in &mut settings.rotation_ops {
                if let TransformQuatOp::YawFollow { threshold, .. } = op
                    && *threshold != wanted
                {
                    *threshold = wanted;
                }
            }
        }
    }

    pub fn tick(
        &mut self,
        world: &World,
        upper: &UpperBodyIkSystem,
        lower: &LowerBodyIkSystem,
        dt_sec: f32,
    ) {
        let dt = if dt_sec.is_finite() {
            dt_sec.max(0.0)
        } else {
            0.0
        };
        for (&id, instance) in &mut self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            let (Some(spine), Some(head), Some(root)) = (
                upper.spine(id),
                avatar.head_mount.and_then(|m| world_matrix(world, m)),
                upper.spine(id).and_then(|s| world_matrix(world, s.root)),
            ) else {
                *instance = BodyInstance::default();
                continue;
            };
            if let Some(height) = lower.feet(id).and_then(|feet| {
                feet.iter()
                    .filter_map(|f| f.support_height)
                    .min_by(f32::total_cmp)
            }) {
                instance.support = Some(height);
            }
            instance.dwell += dt;
            let scale = (0..3).map(|r| root[0][r] * root[0][r]).sum::<f32>().sqrt();
            let head_above = instance.support.map(|floor| head[3][1] - floor);
            let standing = spine.head_rest_height * scale;
            let next = match avatar.posture {
                BodyPosture::Vertical => BodyMode::Vertical,
                BodyPosture::Horizontal => BodyMode::Horizontal,
                BodyPosture::Auto => match (instance.mode, head_above) {
                    (_, None) => instance.mode,
                    _ if instance.dwell < MIN_DWELL_SEC => instance.mode,
                    (BodyMode::Vertical, Some(above)) => {
                        let limited = lower.status(id) == LowerBodyStatus::Limited;
                        if above < ENTER_HEAD_FRACTION * standing
                            || (limited && above < ENTER_LIMITED_HEAD_FRACTION * standing)
                        {
                            BodyMode::Horizontal
                        } else {
                            BodyMode::Vertical
                        }
                    }
                    (BodyMode::Horizontal, Some(above)) => {
                        if above > EXIT_HEAD_FRACTION * standing {
                            BodyMode::Vertical
                        } else {
                            BodyMode::Horizontal
                        }
                    }
                },
            };
            if next != instance.mode {
                instance.mode = next;
                instance.dwell = 0.0;
            }
            let goal = if instance.mode == BodyMode::Horizontal {
                1.0
            } else {
                0.0
            };
            let step = if BLEND_SEC > 0.0 { dt / BLEND_SEC } else { 1.0 };
            instance.weight += (goal - instance.weight).clamp(-step, step);
            // Solve while horizontal. Blending out keeps the last solved
            // pitch and fades it with the weight: re-solving against a rising
            // head would drop it to zero at once while the legs still kneel.
            if instance.mode == BodyMode::Horizontal
                && let Some(floor) = instance.support
            {
                instance.held_pitch = solve_pitch(spine, root, head[3][1], floor, scale);
            }
            if instance.weight <= 0.0 {
                instance.held_pitch = 0.0;
            }
            instance.pitch = instance.weight * instance.held_pitch;
        }
    }
}

fn world_matrix(world: &World, id: ComponentId) -> Option<TransformMatrix> {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| t.transform.matrix_world)
}

/// World-space rise of a root-frame vector.
fn rise(root: TransformMatrix, v: [f32; 3]) -> f32 {
    mat4_mul_vec4(root, [v[0], v[1], v[2], 0.0])[1]
}

/// Height of the hip joints below a fixed head when the torso is pitched by
/// `pitch`. The neck is assumed to follow the upper chest; the head/neck
/// constraint closes the remaining gap by placing the root.
pub(crate) fn hip_joint_height(
    spine: &SpineRig,
    root: TransformMatrix,
    head_y: f32,
    pitch: f32,
) -> f32 {
    let shares = SpineRig::distribute(pitch);
    let cumulative = [
        shares[0],
        shares[0] + shares[1],
        shares[0] + shares[1] + shares[2],
        pitch,
        pitch,
    ];
    let mut head_above_hips = 0.0;
    for (segment, angle) in spine.segments.iter().zip(cumulative) {
        let turned = quat_rotate_vec3(quat_from_axis_angle(spine.pitch_axis, angle), *segment);
        head_above_hips += rise(root, turned);
    }
    let hips_to_joints = quat_rotate_vec3(
        quat_from_axis_angle(spine.pitch_axis, shares[0]),
        spine.hips_to_hip_joints,
    );
    head_y - head_above_hips + rise(root, hips_to_joints)
}

/// Torso pitch that puts the hip joints one thigh length (plus knee clearance)
/// above the floor, so the knees can rest on it. Clamped to [0, MAX_PITCH].
pub(crate) fn solve_pitch(
    spine: &SpineRig,
    root: TransformMatrix,
    head_y: f32,
    floor: f32,
    scale: f32,
) -> f32 {
    let target = floor + (spine.thigh_length + KNEE_RADIUS) * scale;
    let error = |pitch: f32| hip_joint_height(spine, root, head_y, pitch) - target;
    if error(0.0) >= 0.0 {
        return 0.0;
    }
    if error(MAX_PITCH) < 0.0 {
        return MAX_PITCH;
    }
    let (mut low, mut high) = (0.0, MAX_PITCH);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if error(middle) < 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}
