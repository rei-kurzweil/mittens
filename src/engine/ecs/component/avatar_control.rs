use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{Component, ComponentRef};

/// How AVC treats retained eye-tracker gaze while the avatar head is moving.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeadMotionGazePolicy {
    #[default]
    Live,
    Freeze,
}

impl HeadMotionGazePolicy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "live" => Some(Self::Live),
            "freeze" => Some(Self::Freeze),
            _ => None,
        }
    }
}

/// Settings applied to one AVC-generated two-bone arm chain.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmTwoBoneIkConfig {
    pub pole_direction: [f32; 3],
    pub copy_end_rotation: bool,
    pub weight: f32,
    pub forbidden_bend_normal_z_degrees: Vec<[f32; 2]>,
}

impl ArmTwoBoneIkConfig {
    pub fn left_default() -> Self {
        Self::with_pole([-1.0, 0.0, -1.0])
    }

    pub fn right_default() -> Self {
        Self::with_pole([1.0, 0.0, -1.0])
    }

    pub fn with_pole(pole_direction: [f32; 3]) -> Self {
        Self {
            pole_direction,
            copy_end_rotation: true,
            weight: 1.0,
            forbidden_bend_normal_z_degrees: Vec::new(),
        }
    }
}

/// Coordinates all pose drivers for a humanoid avatar.
///
/// **Design rule**: every transform driver that moves this avatar's bones must be a
/// child of (or otherwise routed through) this component.  This includes the primary
/// body/head driver (`Input` / `InputXR`) and any hand controllers (`ControllerXR`).
/// Uncoordinated drivers that bypass this component and write directly to armature bones
/// are the root cause of the torso-rotation bug in the old two-input design.
///
/// Multiple drivers are fine; what matters is that they all appear in this node's
/// subtree so `AvatarControlSystem` can discover and route them during init.
///
/// ## Controller discovery
///
/// Hand controllers are discovered automatically by topology: any enabled
/// `ControllerXRComponent` that is a **direct child** of this component is a candidate hand
/// driver. Its `hand` field (`Left` / `Right`) selects the side, and its first direct
/// `TransformComponent` child is the tracked target written by `OpenXRSystem`.
///
/// AVC creates a side's TwoBoneIK chain only when both the complete mapped arm and that usable
/// tracked target exist. A mapped arm without a controller remains in its FK/rest/animation pose.
///
/// ## Topology (after init)
///
/// ```text
/// Input  (or  InputXR)                    ← primary driver
///   └── driven_t
///         ├── AvatarControlComponent
///               ├── model_root  (TransformComponent, Y offset)
///               │     └── GLTFComponent
///               │           └── [armature]
///               │                 left_upper_arm → left_lower_arm → left_hand
///               │                 right_upper_arm → right_lower_arm → right_hand
///               ├── ControllerXR (Left,  Grip) { controller_driven_t }
///               ├── ControllerXR (Right, Grip) { controller_driven_t }
///               ├── [runtime] left TwoBoneIK → corrected left controller target
///               └── [runtime] right TwoBoneIK → corrected right controller target
///         └── head_mount  ← injected by AVC; fixed offset from driven_t
///               └── J_Bip_C_Head (displaced from the armature)
/// ```
#[derive(Debug, Clone)]
pub struct AvatarControlComponent {
    /// Explicit amplitude observer used for the `viseme_aa` mouth-open fallback.
    pub mouth_open_amplitude: Option<ComponentRef>,
    /// Linear PCM RMS that maps to a closed mouth.
    pub mouth_open_rms_floor: f32,
    /// Linear PCM RMS that maps to a fully open mouth.
    pub mouth_open_rms_ceiling: f32,
    /// Maximum `viseme_aa` weight produced by the amplitude fallback.
    ///
    /// This deliberately controls visual extent independently from the input
    /// calibration: a performer can retain a sensitive response without
    /// necessarily driving the mouth all the way to its morph's full weight.
    pub mouth_open_amount: f32,
    /// Exponential response rate in 1/seconds. Zero disables smoothing.
    pub mouth_open_smoothing: f32,
    pub(crate) resolved_mouth_open_amplitude: Option<ComponentId>,
    pub(crate) mouth_open_weight: f32,
    pub(crate) mouth_open_missing_slot_diagnosed: bool,

    /// Whether AVC should generate an upright collision capsule. Enabled by default.
    pub collision_enabled: bool,

    /// Explicit transform corrected by the generated capsule (e.g. a falling root).
    pub movement_target: Option<ComponentRef>,

    /// Authored character-controller radius, capped to half the measured height.
    pub capsule_radius: f32,

    /// Authored arm policies. `None` uses the side's engine defaults.
    pub left_two_bone_ik: Option<ArmTwoBoneIkConfig>,
    pub right_two_bone_ik: Option<ArmTwoBoneIkConfig>,

    /// Yaw delta (radians) that triggers body rotation. Default: π/4 (45°).
    pub body_yaw_threshold: f32,

    /// Body rotation rate (radians/sec). Default: 3.0.
    pub body_yaw_rate: f32,

    /// Use +Z as the authored forward axis override.
    ///
    /// When not explicitly overridden, AVC keeps the shared XR-style default
    /// (`false`) for both desktop and XR. This override remains available for
    /// assets that were authored with a different convention.
    pub forward_plus_z: bool,

    /// Whether `forward_plus_z` was explicitly authored as an override.
    pub forward_plus_z_overridden: bool,

    /// Initial body yaw (radians) seeded into the `YawFollow` pipeline op.
    ///
    /// When not explicitly overridden, AVC uses the shared default `π`.
    pub initial_body_yaw: f32,

    /// Whether `initial_body_yaw` was explicitly authored as an override.
    pub initial_body_yaw_overridden: bool,

    /// Optional rotation smoothing for hand pose drivers (ControllerXR etc.).
    /// Applied to the rotation channel of each discovered hand driver's pipeline.
    /// Equivalent to `QuatTemporalFilter` smoothing_factor. `None` = no smoothing pipeline.
    pub hand_rotation_smoothing: Option<f32>,

    /// Explicit avatar height (metres) used to set model_root.y = -avatar_height.
    /// Overrides the camera_bone auto-calibration if both are set.
    /// Use this when the camera bone lookup fails or the mesh height is known in advance.
    pub avatar_height: Option<f32>,

    /// Vertical distance (metres) from the head bone pivot to the eyes.
    ///
    /// VRM `J_Bip_C_Head` pivot sits at the skull base; the eye line is typically
    /// ~0.08 m above that.  When this is set, AVC shifts `model_root.y` down by
    /// this amount so the EYES (not the bone pivot) land at `driven_t`'s world Y
    /// — i.e. at HMD height in VR, or at the desktop input height.
    ///
    /// Without this, the avatar's eyes sit above the HMD eye position and the
    /// face/hair mesh swings into the XR camera frustum when pitching down.
    ///
    /// Applies on top of either `camera_bone` auto-calibration or
    /// `avatar_height` override.  Default: `None` (no adjustment).
    pub eye_height_from_head_bone: Option<f32>,

    /// Vertical offset (metres) used exclusively for the head IK target calculation.
    ///
    /// This is decoupled from the camera position transform (`T { CXR }` wrapper)
    /// so the camera can be positioned freely without affecting how the FABRIK solver
    /// bends the spine.  Typically set to a small value like 0.04–0.08 to account for
    /// the gap between the head bone pivot and the eye position, causing the spine to
    /// bend so the head lands at the right height relative to the HMD.
    ///
    /// When set, the FABRIK target_position_offset uses this value (Y-only) instead of
    /// reading the camera transform's translation.  If `None`, no offset is applied to
    /// the IK target (the head bone pivot chases the HMD position directly).
    /// Default: `None`.
    pub head_ik_eye_height: Option<f32>,

    /// Suppress incoming gaze changes during rapid head motion. This is an
    /// opt-in visual mitigation for unstable eye-tracking samples.
    pub head_motion_gaze_policy: HeadMotionGazePolicy,

    // Runtime IDs set by AvatarControlSystem on first tick:
    pub(crate) head_mount: Option<ComponentId>,
    pub(crate) displaced_head: Option<ComponentId>,
    /// Cached left hand bone id (end effector of left-arm TwoBoneIK).
    pub(crate) left_hand_bone_id: Option<ComponentId>,
    /// Cached right hand bone id (end effector of right-arm TwoBoneIK).
    pub(crate) right_hand_bone_id: Option<ComponentId>,
    /// Raw left controller/grip transform that feeds the optional hand offset node.
    pub(crate) left_hand_raw_target_id: Option<ComponentId>,
    /// Raw right controller/grip transform that feeds the optional hand offset node.
    pub(crate) right_hand_raw_target_id: Option<ComponentId>,
    /// Final left visual hand target transform used by IK.
    pub(crate) left_hand_visual_target_id: Option<ComponentId>,
    /// Final right visual hand target transform used by IK.
    pub(crate) right_hand_visual_target_id: Option<ComponentId>,
    /// Immutable rest-pose correction that maps controller aim onto the finger mount.
    pub(crate) left_hand_aim_correction: Option<[f32; 4]>,
    pub(crate) right_hand_aim_correction: Option<[f32; 4]>,

    /// ComponentId of the body pipeline root (`TransformForkTRSComponent`).
    /// Set by `try_init_splices`.
    pub(crate) body_pipeline_id: Option<ComponentId>,

    /// The mapped/generated transform that owns the camera path after initialization.
    pub(crate) splice_camera_bone: Option<ComponentId>,
    pub(crate) humanoid_map_gltf: Option<ComponentId>,
    pub(crate) humanoid_map_generation: u64,
    /// Eye bones currently owned by AVC's automatic eye-tracking driver.
    pub(crate) left_eye_tracking_bone_id: Option<ComponentId>,
    pub(crate) right_eye_tracking_bone_id: Option<ComponentId>,
    pub(crate) last_eye_gaze_basis_rotation: Option<[f32; 4]>,
    pub(crate) eye_gaze_frozen: bool,
    pub(crate) eye_gaze_still_time_sec: f32,
    pub(crate) frozen_left_eye_gaze: Option<[f32; 3]>,
    pub(crate) frozen_right_eye_gaze: Option<[f32; 3]>,

    /// Debug/diagnostic flag: skip creation of the body-rotation pipeline entirely.
    /// When `true`, model_root stays directly under AVC and only head rotation is applied.
    /// Use this to isolate whether torso-twist bugs originate in the body pipeline.
    pub skip_body_pipeline: bool,

    /// Debug/diagnostic flag: when enabled, arm TwoBoneIK chains spawn overlay
    /// visualizations for the actual target vector, transformed pole vector,
    /// bend-plane normal, and solved elbow direction used by the solver.
    pub ik_debug: bool,

    // ---------------------------------------------------------------------
    // Head-pose-sensitive body XZ translate follow (see
    // `docs/task/avatar-control-simple-humanoid-body-follow.md`, Phase 1).
    // ---------------------------------------------------------------------
    /// Enables the optional neck rest-pin when the map resolves a neck slot.
    pub neck_pin_enabled: bool,

    // Runtime state set by AvatarControlSystem / HeadPoseBodyXzFollowSystem:
    /// `model_root` component id, stashed at init so the body-follow system
    /// doesn't have to re-walk topology each tick.
    pub(crate) model_root_id: Option<ComponentId>,

    /// `model_root.local.translation.y` at rest (body height offset).  Set
    /// Set once at init from resolved camera-anchor calibration or `avatar_height`.
    pub(crate) model_root_local_y: f32,

    /// Resolved neck bone id (under `model_root`).  `None` if not found.
    pub(crate) neck_bone_id: Option<ComponentId>,

    /// Neck rest local translation cached at init for the rest-pin.
    pub(crate) neck_rest_translation: Option<[f32; 3]>,

    /// Runtime-only generated upright capsule transform.
    pub(crate) capsule_transform_id: Option<ComponentId>,

    /// Runtime-only generated collidable, used to refresh XR movement routing.
    pub(crate) capsule_collidable_id: Option<ComponentId>,

    component: Option<ComponentId>,
}

impl AvatarControlComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mouth_open_from_amplitude(mut self, source: ComponentRef) -> Self {
        self.mouth_open_amplitude = Some(source);
        self.resolved_mouth_open_amplitude = None;
        self.mouth_open_weight = 0.0;
        self
    }

    pub fn with_mouth_open_rms_floor(mut self, floor: f32) -> Result<Self, String> {
        if !floor.is_finite() || floor < 0.0 || floor >= self.mouth_open_rms_ceiling {
            return Err("AvatarControl.mouth_open_rms_floor requires 0 <= floor < ceiling".into());
        }
        self.mouth_open_rms_floor = floor;
        Ok(self)
    }

    pub fn with_mouth_open_rms_ceiling(mut self, ceiling: f32) -> Result<Self, String> {
        if !ceiling.is_finite() || ceiling <= self.mouth_open_rms_floor {
            return Err("AvatarControl.mouth_open_rms_ceiling requires ceiling > floor".into());
        }
        self.mouth_open_rms_ceiling = ceiling;
        Ok(self)
    }

    /// Configure the amplitude response as a centre RMS and full RMS range.
    ///
    /// This is equivalent to setting the floor and ceiling directly, but is
    /// often more natural when the useful microphone level is known first:
    /// `floor = center_rms - range_rms / 2`,
    /// `ceiling = center_rms + range_rms / 2`.
    pub fn with_mouth_open_rms_center_range(
        mut self,
        center_rms: f32,
        range_rms: f32,
    ) -> Result<Self, String> {
        if !center_rms.is_finite() || center_rms <= 0.0 {
            return Err(
                "AvatarControl.mouth_open_rms_center_range requires a finite center_rms > 0".into(),
            );
        }
        if !range_rms.is_finite() || range_rms <= 0.0 || range_rms * 0.5 > center_rms {
            return Err(
                "AvatarControl.mouth_open_rms_center_range requires finite range_rms > 0 with range_rms / 2 <= center_rms".into(),
            );
        }
        let half_range = range_rms * 0.5;
        self.mouth_open_rms_floor = center_rms - half_range;
        self.mouth_open_rms_ceiling = center_rms + half_range;
        Ok(self)
    }

    /// Set the maximum visual mouth-open contribution of the amplitude fallback.
    pub fn with_mouth_open_amount(mut self, amount: f32) -> Result<Self, String> {
        if !amount.is_finite() || !(0.0..=1.0).contains(&amount) {
            return Err("AvatarControl.mouth_open_amount requires a finite value in 0..=1".into());
        }
        self.mouth_open_amount = amount;
        Ok(self)
    }

    pub fn with_mouth_open_smoothing(mut self, rate: f32) -> Result<Self, String> {
        if !rate.is_finite() || rate < 0.0 {
            return Err(
                "AvatarControl.mouth_open_smoothing requires a finite non-negative rate".into(),
            );
        }
        self.mouth_open_smoothing = rate;
        Ok(self)
    }

    pub fn with_collision_disabled(mut self) -> Self {
        self.collision_enabled = false;
        self
    }

    pub fn with_capsule_radius(mut self, radius: f32) -> Self {
        self.capsule_radius = radius.max(0.0);
        self
    }

    pub fn with_left_two_bone_ik(mut self, config: ArmTwoBoneIkConfig) -> Self {
        self.left_two_bone_ik = Some(config);
        self
    }

    pub fn with_right_two_bone_ik(mut self, config: ArmTwoBoneIkConfig) -> Self {
        self.right_two_bone_ik = Some(config);
        self
    }

    pub fn with_body_yaw_threshold(mut self, t: f32) -> Self {
        self.body_yaw_threshold = t;
        self
    }

    pub fn with_body_yaw_rate(mut self, r: f32) -> Self {
        self.body_yaw_rate = r;
        self
    }

    /// Override the initial body yaw (radians) seeded into the `YawFollow` pipeline op.
    /// Use `std::f32::consts::PI` for rigs that face -Z at rest.
    pub fn with_initial_yaw(mut self, yaw: f32) -> Self {
        self.initial_body_yaw = yaw;
        self.initial_body_yaw_overridden = true;
        self
    }

    /// Use +Z as the authored forward axis override.
    pub fn with_forward_plus_z(mut self) -> Self {
        self.forward_plus_z = true;
        self.forward_plus_z_overridden = true;
        self
    }

    /// Enable rotation smoothing for hand pose drivers.
    /// Set to e.g. `220.0` for smooth VR controller rotation.
    pub fn with_hand_rotation_smoothing(mut self, factor: f32) -> Self {
        self.hand_rotation_smoothing = Some(factor);
        self
    }

    /// Skip creation of the body-rotation pipeline. Only head rotation will be applied.
    /// Use to isolate whether torso-twist bugs originate in the body pipeline.
    pub fn with_body_pipeline_disabled(mut self) -> Self {
        self.skip_body_pipeline = true;
        self
    }

    /// Enable TwoBoneIK debug visualizations for chains owned by this AVC.
    pub fn with_ik_debug(mut self) -> Self {
        self.ik_debug = true;
        self
    }

    /// Explicitly set `model_root.y = -height` during init, bypassing mapped
    /// camera-anchor calibration. Use when the mesh height is known in advance.
    /// Camera re-parenting still uses the resolved humanoid camera anchor.
    pub fn with_avatar_height(mut self, height: f32) -> Self {
        self.avatar_height = Some(height);
        self
    }

    /// Shift `model_root.y` down so the avatar's EYES (not the head bone pivot)
    /// land at `driven_t`'s world Y.  Default eye offset for VRM is ~0.08.
    pub fn with_eye_height_from_head_bone(mut self, dy: f32) -> Self {
        self.eye_height_from_head_bone = Some(dy);
        self
    }

    /// Disable the neck rest-pin.
    pub fn without_neck_pin(mut self) -> Self {
        self.neck_pin_enabled = false;
        self
    }

    pub fn with_neck_pin_enabled(mut self, enabled: bool) -> Self {
        self.neck_pin_enabled = enabled;
        self
    }

    /// Set the vertical offset for the head IK target calculation (metres).
    /// Decoupled from the camera position so spine bending and camera positioning
    /// can be controlled independently. Default: `None`.
    pub fn with_head_ik_eye_height(mut self, dy: f32) -> Self {
        self.head_ik_eye_height = Some(dy);
        self
    }

    pub fn with_head_motion_gaze_policy(mut self, policy: HeadMotionGazePolicy) -> Self {
        self.head_motion_gaze_policy = policy;
        self
    }
}

impl Default for AvatarControlComponent {
    fn default() -> Self {
        Self {
            mouth_open_amplitude: None,
            mouth_open_rms_floor: 0.015,
            mouth_open_rms_ceiling: 0.12,
            mouth_open_amount: 1.0,
            mouth_open_smoothing: 18.0,
            resolved_mouth_open_amplitude: None,
            mouth_open_weight: 0.0,
            mouth_open_missing_slot_diagnosed: false,
            collision_enabled: true,
            movement_target: None,
            capsule_radius: 0.28,
            left_two_bone_ik: None,
            right_two_bone_ik: None,
            body_yaw_threshold: std::f32::consts::FRAC_PI_4,
            body_yaw_rate: 3.0,
            forward_plus_z: false,
            forward_plus_z_overridden: false,
            initial_body_yaw: 0.0,
            initial_body_yaw_overridden: false,
            hand_rotation_smoothing: None,
            avatar_height: None,
            eye_height_from_head_bone: None,
            head_motion_gaze_policy: HeadMotionGazePolicy::Live,
            head_mount: None,
            displaced_head: None,
            left_hand_bone_id: None,
            right_hand_bone_id: None,
            left_hand_raw_target_id: None,
            right_hand_raw_target_id: None,
            left_hand_visual_target_id: None,
            right_hand_visual_target_id: None,
            left_hand_aim_correction: None,
            right_hand_aim_correction: None,
            body_pipeline_id: None,
            splice_camera_bone: None,
            humanoid_map_gltf: None,
            humanoid_map_generation: 0,
            left_eye_tracking_bone_id: None,
            right_eye_tracking_bone_id: None,
            last_eye_gaze_basis_rotation: None,
            eye_gaze_frozen: false,
            eye_gaze_still_time_sec: 0.0,
            frozen_left_eye_gaze: None,
            frozen_right_eye_gaze: None,
            skip_body_pipeline: false,
            ik_debug: false,
            head_ik_eye_height: None,
            neck_pin_enabled: true,
            model_root_id: None,
            model_root_local_y: 0.0,
            neck_bone_id: None,
            neck_rest_translation: None,
            capsule_transform_id: None,
            capsule_collidable_id: None,
            component: None,
        }
    }
}

impl Component for AvatarControlComponent {
    fn name(&self) -> &'static str {
        "avatar_control"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterAvatarControl {
                component_id: component,
            },
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::scripting::ast::{Expression, Ident, TableFieldValue};
        let mut c = ce("AvatarControl")
            .with_call(
                "body_yaw_threshold",
                vec![num(self.body_yaw_threshold as f64)],
            )
            .with_call("body_yaw_rate", vec![num(self.body_yaw_rate as f64)]);
        if !self.collision_enabled {
            c = c.with_call("collision_disabled", vec![]);
        }
        if self.head_motion_gaze_policy == HeadMotionGazePolicy::Freeze {
            c = c.with_call("head_motion_gaze_policy", vec![s("freeze")]);
        }
        if (self.capsule_radius - 0.28).abs() > f32::EPSILON {
            c = c.with_call("capsule_radius", vec![num(self.capsule_radius as f64)]);
        }
        let ik_table = |config: &ArmTwoBoneIkConfig| {
            let field = |name: &str, value| TableFieldValue {
                name: Ident(name.into()),
                value,
            };
            Expression::Table(vec![
                field(
                    "pole_direction",
                    array(nums(config.pole_direction.iter().map(|&v| v as f64))),
                ),
                field("copy_end_rotation", b(config.copy_end_rotation)),
                field("weight", num(config.weight as f64)),
                field(
                    "forbidden_bend_normal_z_degrees",
                    array(
                        config
                            .forbidden_bend_normal_z_degrees
                            .iter()
                            .map(|range| array(nums(range.iter().map(|&v| v as f64))))
                            .collect(),
                    ),
                ),
            ])
        };
        if let Some(config) = &self.left_two_bone_ik {
            c = c.with_call("left_two_bone_ik", vec![ik_table(config)]);
        }
        if let Some(config) = &self.right_two_bone_ik {
            c = c.with_call("right_two_bone_ik", vec![ik_table(config)]);
        }
        if self.forward_plus_z_overridden && self.forward_plus_z {
            c = c.with_call("forward_plus_z", vec![]);
        }
        if self.initial_body_yaw_overridden {
            c = c.with_call("initial_yaw", vec![num(self.initial_body_yaw as f64)]);
        }
        if self.ik_debug {
            c = c.with_call("ik_debug", vec![]);
        }
        if let Some(factor) = self.hand_rotation_smoothing {
            c = c.with_call("hand_rotation_smoothing", vec![num(factor as f64)]);
        }
        if let Some(h) = self.avatar_height {
            c = c.with_call("avatar_height", vec![num(h as f64)]);
        }
        if let Some(dy) = self.eye_height_from_head_bone {
            c = c.with_call("eye_height_from_head_bone", vec![num(dy as f64)]);
        }
        if let Some(dy) = self.head_ik_eye_height {
            c = c.with_call("head_ik_eye_height", vec![num(dy as f64)]);
        }
        if !self.neck_pin_enabled {
            c = c.with_call("neck_pin_disabled", vec![]);
        }
        if let Some(source) = &self.movement_target {
            let source = match source {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            };
            c = c.with_call("movement_target", vec![source]);
        }
        if let Some(source) = &self.mouth_open_amplitude {
            let source = match source {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            };
            c = c.with_call("mouth_open_from_amplitude", vec![source]);
        }
        if (self.mouth_open_rms_floor - 0.015).abs() > f32::EPSILON {
            c = c.with_call(
                "mouth_open_rms_floor",
                vec![num(self.mouth_open_rms_floor as f64)],
            );
        }
        if (self.mouth_open_rms_ceiling - 0.12).abs() > f32::EPSILON {
            c = c.with_call(
                "mouth_open_rms_ceiling",
                vec![num(self.mouth_open_rms_ceiling as f64)],
            );
        }
        if (self.mouth_open_amount - 1.0).abs() > f32::EPSILON {
            c = c.with_call(
                "mouth_open_amount",
                vec![num(self.mouth_open_amount as f64)],
            );
        }
        if (self.mouth_open_smoothing - 18.0).abs() > f32::EPSILON {
            c = c.with_call(
                "mouth_open_smoothing",
                vec![num(self.mouth_open_smoothing as f64)],
            );
        }
        c
    }
}
