//! Body solver: the one place posture is decided. It picks the body mode for
//! each avatar and the torso pitch that mode needs. The upper body solver
//! writes the torso bones from that pitch; the lower body solver reads the
//! mode to choose standing or kneeling leg goals.
use super::lower_body::{LowerBodyIkSystem, LowerBodyStatus};
use super::upper_body::{SpineRig, UpperBodyIkSystem};
use crate::engine::ecs::component::{AvatarControlComponent, BodyPosture, TransformComponent};
use crate::engine::ecs::system::AttachmentSystem;
use crate::engine::ecs::system::input_xr_flying::is_flying;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{mat4_mul_vec4, quat_from_axis_angle, quat_rotate_vec3};
use std::collections::HashMap;

mod debug_overlay;
mod support_outline;

/// Vertical is everything an upright avatar does today. Horizontal is on hands
/// and knees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BodyMode {
    #[default]
    Vertical,
    Horizontal,
}

impl BodyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }

    /// The pose each mode currently produces. Horizontal will gain lying
    /// poses (stomach, back) beside hands and knees.
    pub fn pose(self) -> &'static str {
        match self {
            Self::Vertical => "standing",
            Self::Horizontal => "hands_and_knees",
        }
    }
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
    debug: debug_overlay::DebugOverlay,
}

#[derive(Debug, Default)]
struct BodyInstance {
    mode: BodyMode,
    /// 0 = fully vertical, 1 = fully horizontal.
    weight: f32,
    dwell: f32,
    support: Option<f32>,
    mounted: bool,
    /// Last pitch solved in horizontal mode, faded out by `weight`.
    held_pitch: f32,
    pitch: f32,
    /// Time since the last `CAT_DEBUG_BODY_SOLVER` line.
    debug_elapsed: f32,
}

fn debug_body_solver_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("CAT_DEBUG_BODY_SOLVER")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
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
    /// A paused avatar's body holds its heading (rate 0).
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
            let wanted_rate = if avatar.paused { 0.0 } else { avatar.body_yaw_rate };
            let Some(settings) = world
                .get_component_by_id_as_mut::<TransformOperatorComponent>(operator)
                .and_then(|o| o.fork_trs_settings_mut())
            else {
                continue;
            };
            for op in &mut settings.rotation_ops {
                if let TransformQuatOp::YawFollow { threshold, rate, .. } = op {
                    if *threshold != wanted {
                        *threshold = wanted;
                    }
                    if *rate != wanted_rate {
                        *rate = wanted_rate;
                    }
                }
            }
        }
    }

    /// Horizontal posture collides as a low crawling sphere, vertical as
    /// the standing capsule (`set_crawling_collider`).
    pub fn apply_posture_colliders(&self, world: &mut World) {
        for (&id, instance) in &self.avatars {
            crate::engine::ecs::system::avatar_control_system::capsule::set_crawling_collider(
                world,
                id,
                instance.mode == BodyMode::Horizontal,
            );
        }
    }

    pub fn tick(
        &mut self,
        world: &World,
        emit: &mut dyn SignalEmitter,
        upper: &UpperBodyIkSystem,
        lower: &LowerBodyIkSystem,
        attachments: &AttachmentSystem,
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
            let mounted = attachments.is_avatar_mounted(id);
            let flying = !mounted && avatar.posture == BodyPosture::Auto && is_flying(world, id);
            let feet = (!mounted && !flying).then(|| lower.feet(id)).flatten();
            let sampled = feet.and_then(|feet| {
                feet.iter()
                    .filter_map(|foot| foot.support_height.map(|height| (height, foot.support)))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            });
            let knees = !flying && instance.weight >= 0.5;
            if mounted && !instance.mounted {
                // Feet stop reporting support while riding. The seat may carry
                // the rider to an entirely different floor before dismount.
                instance.support = None;
                instance.held_pitch = 0.0;
                instance.weight = 0.0;
            } else if flying {
                instance.support = None;
            } else if let Some((height, _)) = sampled {
                instance.support = Some(height);
            }
            instance.mounted = mounted;
            instance.dwell += dt;
            let scale = (0..3).map(|r| root[0][r] * root[0][r]).sum::<f32>().sqrt();
            let head_above = instance.support.map(|floor| head[3][1] - floor);
            let standing = spine.head_rest_height * scale;
            let lower_status = lower.status(id);
            let enter = if lower_status == LowerBodyStatus::Limited {
                ENTER_LIMITED_HEAD_FRACTION * standing
            } else {
                ENTER_HEAD_FRACTION * standing
            };
            let exit = EXIT_HEAD_FRACTION * standing;
            let (next, reason) = match avatar.posture {
                BodyPosture::Vertical => (BodyMode::Vertical, "forced"),
                BodyPosture::Horizontal => (BodyMode::Horizontal, "forced"),
                BodyPosture::Auto if mounted => (BodyMode::Vertical, "mounted"),
                BodyPosture::Auto if flying => (BodyMode::Vertical, "flight"),
                // Pitching relies on the head/neck constraint to place the
                // root under the head. Legacy body-follow keeps the root a
                // fixed height below the head, so a pitched torso would sink
                // the hips through the floor and stretch the neck.
                BodyPosture::Auto if !avatar.neck_pin_enabled || !avatar.head_neck_body_enabled => {
                    (BodyMode::Vertical, "no_neck_constraint")
                }
                BodyPosture::Auto => match (instance.mode, head_above) {
                    (_, None) => (instance.mode, "no floor"),
                    _ if instance.dwell < MIN_DWELL_SEC => (instance.mode, "minimum dwell"),
                    (BodyMode::Vertical, Some(above)) => {
                        if above < ENTER_HEAD_FRACTION * standing {
                            (BodyMode::Horizontal, "head_height")
                        } else if above < enter {
                            (BodyMode::Horizontal, "crouch_exhausted")
                        } else {
                            (BodyMode::Vertical, "above crawl threshold")
                        }
                    }
                    (BodyMode::Horizontal, Some(above)) => {
                        if above > exit {
                            (BodyMode::Vertical, "head_height")
                        } else {
                            (BodyMode::Horizontal, "below stand threshold")
                        }
                    }
                },
            };
            if next != instance.mode {
                emit.push_event(
                    id,
                    EventSignal::PostureChanged {
                        avatar: id,
                        mode: next.as_str(),
                        previous: instance.mode.as_str(),
                        pose: next.pose(),
                        reason,
                    },
                );
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
            if self.debug.target == Some(id) {
                let source_foot = sampled.and_then(|(height, _)| {
                    feet?.iter().find(|foot| foot.support_height == Some(height) && foot.support.is_some())
                });
                let contacts = feet.map_or([None; 2], |feet| {
                    feet.map(|foot| {
                        let point = if knees { foot.knee } else { foot.solved_sole };
                        foot.support_height
                            .or(instance.support)
                            .map(|floor| debug_overlay::Contact {
                                point,
                                floor,
                                sampled: foot.support.is_some(),
                            })
                    })
                });
                self.debug.sample = Some(debug_overlay::Sample {
                    head: [head[3][0], head[3][1], head[3][2]],
                    standing,
                    floor: instance.support,
                    source: source_foot.and_then(|foot| foot.support),
                    surface: source_foot.and_then(|foot| lower.support_surface(id, foot)),
                    contacts,
                    enter,
                    exit,
                    mode: instance.mode,
                    mounted,
                });
            }
            if debug_body_solver_enabled() {
                instance.debug_elapsed += dt;
                if instance.debug_elapsed >= 0.5 {
                    instance.debug_elapsed = 0.0;
                    // Compare a peer's local line with its remote copy on the
                    // other machine: same head, so any input gap shows here.
                    let hip_target = instance
                        .support
                        .map(|floor| floor + (spine.thigh_length + KNEE_RADIUS) * scale);
                    let knees = lower
                        .feet(id)
                        .map(|feet| [feet[0].knee[1], feet[1].knee[1]]);
                    // Wall clock, so lines from both machines line up.
                    let unix_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|t| t.as_millis())
                        .unwrap_or(0);
                    eprintln!(
                        "[body] t={unix_ms} {} mode={} weight={:.2} pitch={:.2} head_y={:.3} root_y={:.3} floor={:?} hip_y={:.3} hip_target={:?} knee_y={:?} scale={:.3} lower={:?}",
                        world.component_label(id).unwrap_or("?"),
                        instance.mode.as_str(),
                        instance.weight,
                        instance.pitch,
                        head[3][1],
                        root[3][1],
                        instance.support,
                        hip_joint_height(spine, root, head[3][1], instance.pitch),
                        hip_target,
                        knees,
                        scale,
                        lower.status(id),
                    );
                }
            }
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
/// `pitch`. The neck is assumed to follow the last torso bone; the head/neck
/// constraint closes the remaining gap by placing the root.
pub(crate) fn hip_joint_height(
    spine: &SpineRig,
    root: TransformMatrix,
    head_y: f32,
    pitch: f32,
) -> f32 {
    // Each segment turns by the pitch of every torso bone below its end:
    // segment i runs from bone i, and the neck and head carry the full pitch.
    let shares = spine.distribute(pitch);
    let mut head_above_hips = 0.0;
    let mut turned_so_far = 0.0;
    for (i, segment) in spine.segments().iter().enumerate() {
        turned_so_far += shares.get(i).copied().unwrap_or(0.0);
        let angle = if i < spine.count {
            turned_so_far
        } else {
            pitch
        };
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
