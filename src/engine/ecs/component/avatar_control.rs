use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

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
/// Hand controllers are discovered automatically by topology: any `ControllerXRComponent`
/// that is a **direct child** of this component is registered as a hand driver.
/// Its `hand` field (`Left` / `Right`) determines which hand bone it drives.
/// The bone is displaced under the controller's first `TransformComponent` child
/// (the driven transform written by `OpenXRSystem`).
///
/// If no controller is present for a configured hand bone, a plain
/// `TransformComponent` splice is inserted instead (for IK-only or static setups).
///
/// ## Topology (after init)
///
/// ```text
/// Input  (or  InputXR)                    ← primary driver
///   └── driven_t
///         └── AvatarControlComponent
///               ├── model_root  (TransformComponent, Y offset)
///               │     └── GLTFComponent
///               │           └── [armature]
///               │                 J_Bip_C_Neck
///               │                   └── splice_head  ← injected by system
///               │                         └── J_Bip_C_Head (displaced; aim-driven by driven_t)
///               │                 left_lower_arm
///               │                   └── ControllerXR (Left, Grip)  ← moved here by system
///               │                         └── controller_driven_t
///               │                               └── J_Bip_L_Hand (displaced)
///               │                 right_lower_arm
///               │                   └── ControllerXR (Right, Grip)
///               │                         └── controller_driven_t
///               │                               └── J_Bip_R_Hand (displaced)
///               ├── ControllerXR (Left,  Grip) { T }  ← declared here; re-parented on init
///               └── ControllerXR (Right, Grip) { T }
/// ```
#[derive(Debug, Clone)]
pub struct AvatarControlComponent {
    /// Name of the bone to displace for head rotation. Default: "J_Bip_C_Head".
    ///
    /// This bone receives the HMD/Input world rotation directly via an `AimConstraint`
    /// IK chain. Rotating the head bone (not the neck) is critical for VR/desktop:
    /// rotating the neck twists the entire torso from the neck up, which looks wrong.
    /// The head's rotation is isolated from the spine so the body can yaw-follow
    /// underneath independently.
    pub head_bone: String,

    /// Name of the left hand bone to splice. `None` = no left hand splice.
    pub left_hand_bone: Option<String>,

    /// Name of the right hand bone to splice. `None` = no right hand splice.
    pub right_hand_bone: Option<String>,

    /// Explicit left upper arm bone name for TwoBoneIK.
    /// If `None` and `left_hand_bone` is set, topology derivation fills it in.
    pub left_upper_arm_bone: Option<String>,

    /// Explicit left lower arm bone name for TwoBoneIK.
    /// If `None` and `left_hand_bone` is set, topology derivation fills it in.
    pub left_lower_arm_bone: Option<String>,

    /// Explicit right upper arm bone name for TwoBoneIK.
    /// If `None` and `right_hand_bone` is set, topology derivation fills it in.
    pub right_upper_arm_bone: Option<String>,

    /// Explicit right lower arm bone name for TwoBoneIK.
    /// If `None` and `right_hand_bone` is set, topology derivation fills it in.
    pub right_lower_arm_bone: Option<String>,

    /// Yaw delta (radians) that triggers body rotation. Default: π/4 (45°).
    pub body_yaw_threshold: f32,

    /// Body rotation rate (radians/sec). Default: 3.0.
    pub body_yaw_rate: f32,

    /// Use +Z as the forward axis (desktop). Default false = -Z (OpenXR).
    pub forward_plus_z: bool,

    /// Initial body yaw (radians) seeded into the `YawFollow` pipeline op.
    /// Set to `π` for VR setups (OpenXR -Z forward at rest). Default: 0.0.
    pub initial_body_yaw: f32,

    /// Optional rotation smoothing for hand pose drivers (ControllerXR etc.).
    /// Applied to the rotation channel of each discovered hand driver's pipeline.
    /// Equivalent to `QuatTemporalFilter` smoothing_factor. `None` = no smoothing pipeline.
    pub hand_rotation_smoothing: Option<f32>,

    /// Bone used as the camera anchor and as the source for auto-calibrating model_root.y.
    ///
    /// When set, `AvatarControlSystem` will:
    ///   1. Measure this bone's local Y height above model_root in the GLTF rest pose.
    ///   2. Override model_root's Y translation to `-bone_local_y`, so the bone sits
    ///      exactly at `driven_t`'s world position (= HMD height in XR; body origin on desktop).
    ///   3. Re-parent any `Camera3DComponent` or `CameraXRComponent` direct children of
    ///      this AVC under this bone, giving them the bone's world transform each tick.
    ///
    /// Typically the same as `head_bone` (e.g. `"J_Bip_C_Head"`) so the camera
    /// inherits both the head's world position (eye height) and rotation.
    /// If `None`, no auto-calibration or camera re-parenting is performed.
    pub camera_bone: Option<String>,

    /// Explicit avatar height (metres) used to set model_root.y = -avatar_height.
    /// Overrides the camera_bone auto-calibration if both are set.
    /// Use this when the camera bone lookup fails or the mesh height is known in advance.
    pub avatar_height: Option<f32>,

    /// Name of the hips bone — the FABRIK spine chain root.  Default: `"J_Bip_C_Hips"`.
    ///
    /// Resolved against `model_root` once during `try_init_splices`.  If `None` or
    /// not found, no spine FABRIK chain is wired — head bone falls back to FK
    /// (visible detachment from neck under pitch).
    pub hips_bone: Option<String>,

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

    // Runtime IDs set by AvatarControlSystem on first tick:
    pub(crate) splice_head:          Option<ComponentId>,
    pub(crate) displaced_head:       Option<ComponentId>,
    /// Immediate parent of the displaced left hand bone (controller's driven_t or plain TC).
    pub(crate) splice_left_hand:     Option<ComponentId>,
    pub(crate) displaced_left_hand:  Option<ComponentId>,
    /// Immediate parent of the displaced right hand bone.
    pub(crate) splice_right_hand:    Option<ComponentId>,
    pub(crate) displaced_right_hand: Option<ComponentId>,

    /// ComponentId of the body pipeline root (`TransformForkTRSComponent`).
    /// Set by `try_init_splices`.
    pub(crate) body_pipeline_id: Option<ComponentId>,

    /// The bone component that cameras were re-parented under (= `camera_bone` lookup result).
    /// Set by `try_init_splices` when `camera_bone` is `Some`.
    pub(crate) splice_camera_bone: Option<ComponentId>,

    /// Debug/diagnostic flag: skip creation of the body-rotation pipeline entirely.
    /// When `true`, model_root stays directly under AVC and only head rotation is applied.
    /// Use this to isolate whether torso-twist bugs originate in the body pipeline.
    pub skip_body_pipeline: bool,

    // ---------------------------------------------------------------------
    // Head-pose-sensitive body XZ translate follow (see
    // `docs/task/avatar-control-simple-humanoid-body-follow.md`, Phase 1).
    // ---------------------------------------------------------------------

    /// Name of the neck bone used by the Phase 2 rest-pin.  When set and
    /// the bone is found under `model_root`, the body-follow system records
    /// its rest local translation at init and restores it each tick if any
    /// other system perturbs it.  Default: `"J_Bip_C_Neck"`.
    pub neck_bone: Option<String>,

    // Runtime state set by AvatarControlSystem / HeadPoseBodyXzFollowSystem:
    /// `model_root` component id, stashed at init so the body-follow system
    /// doesn't have to re-walk topology each tick.
    pub(crate) model_root_id: Option<ComponentId>,

    /// `model_root.local.translation.y` at rest (body height offset).  Set
    /// once at init from `camera_bone` auto-calibration or `avatar_height`.
    pub(crate) model_root_local_y: f32,

    /// Additional body-local translation offset applied on top of the
    /// driver-aligned body placement.
    pub body_to_head_offset: [f32; 3],

    /// Resolved neck bone id (under `model_root`).  `None` if not found.
    pub(crate) neck_bone_id: Option<ComponentId>,

    /// Neck rest local translation cached at init for the rest-pin.
    pub(crate) neck_rest_translation: Option<[f32; 3]>,

    component: Option<ComponentId>,
}

impl AvatarControlComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_head_bone(mut self, name: impl Into<String>) -> Self {
        self.head_bone = name.into();
        self
    }

    pub fn with_left_hand_bone(mut self, name: impl Into<String>) -> Self {
        self.left_hand_bone = Some(name.into());
        self
    }

    pub fn with_right_hand_bone(mut self, name: impl Into<String>) -> Self {
        self.right_hand_bone = Some(name.into());
        self
    }

    pub fn with_left_upper_arm_bone(mut self, name: impl Into<String>) -> Self {
        self.left_upper_arm_bone = Some(name.into());
        self
    }

    pub fn with_left_lower_arm_bone(mut self, name: impl Into<String>) -> Self {
        self.left_lower_arm_bone = Some(name.into());
        self
    }

    pub fn with_right_upper_arm_bone(mut self, name: impl Into<String>) -> Self {
        self.right_upper_arm_bone = Some(name.into());
        self
    }

    pub fn with_right_lower_arm_bone(mut self, name: impl Into<String>) -> Self {
        self.right_lower_arm_bone = Some(name.into());
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
    /// Use `std::f32::consts::PI` for VR setups where the model faces -Z at rest.
    /// Default: 0.0 (model faces +Z, standard for `forward_plus_z` desktop setups).
    pub fn with_initial_yaw(mut self, yaw: f32) -> Self {
        self.initial_body_yaw = yaw;
        self
    }

    /// Use +Z as the forward axis. Required for desktop setups using
    /// `InputTransformModeComponent::forward_z()`.
    pub fn with_forward_plus_z(mut self) -> Self {
        self.forward_plus_z = true;
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

    /// Set the bone used as the camera anchor and for auto-calibrating `model_root.y`.
    ///
    /// `AvatarControlSystem` will measure this bone's local Y in the rest pose and set
    /// `model_root.y = -bone_local_y` so the bone sits at `driven_t`'s world position.
    /// Any `Camera3DComponent` or `CameraXRComponent` direct children of this AVC are
    /// re-parented under this bone during init.
    pub fn with_camera_bone(mut self, name: impl Into<String>) -> Self {
        self.camera_bone = Some(name.into());
        self
    }

    /// Explicitly set `model_root.y = -height` during init, bypassing camera_bone
    /// auto-calibration.  Use when the bone lookup is unreliable or the mesh height
    /// is known in advance.  Camera re-parenting still uses `camera_bone` if set.
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

    /// Set the hips bone name — root of the spine FABRIK chain.
    /// Default (when unset): `"J_Bip_C_Hips"`.
    pub fn with_hips_bone(mut self, name: impl Into<String>) -> Self {
        self.hips_bone = Some(name.into());
        self
    }

    /// Override the neck bone name used by the Phase 2 rest-pin.  Pass `None`
    /// to disable the pin entirely.
    pub fn with_neck_bone(mut self, name: impl Into<String>) -> Self {
        self.neck_bone = Some(name.into());
        self
    }

    /// Disable the neck rest-pin.
    pub fn without_neck_pin(mut self) -> Self {
        self.neck_bone = None;
        self
    }

    /// Set the vertical offset for the head IK target calculation (metres).
    /// Decoupled from the camera position so spine bending and camera positioning
    /// can be controlled independently. Default: `None`.
    pub fn with_head_ik_eye_height(mut self, dy: f32) -> Self {
        self.head_ik_eye_height = Some(dy);
        self
    }

    /// Additional translation on the body in its local transform space.
    pub fn with_body_to_head_offset(mut self, offset: [f32; 3]) -> Self {
        self.body_to_head_offset = offset;
        self
    }
}

impl Default for AvatarControlComponent {
    fn default() -> Self {
        Self {
            head_bone: "J_Bip_C_Head".to_string(),
            left_hand_bone: None,
            right_hand_bone: None,
            left_upper_arm_bone: None,
            left_lower_arm_bone: None,
            right_upper_arm_bone: None,
            right_lower_arm_bone: None,
            body_yaw_threshold: std::f32::consts::FRAC_PI_4,
            body_yaw_rate: 3.0,
            forward_plus_z: false,
            initial_body_yaw: 0.0,
            hand_rotation_smoothing: None,
            camera_bone: None,
            avatar_height: None,
            hips_bone: None,
            eye_height_from_head_bone: None,
            splice_head: None,
            displaced_head: None,
            splice_left_hand: None,
            displaced_left_hand: None,
            splice_right_hand: None,
            displaced_right_hand: None,
            body_pipeline_id: None,
            splice_camera_bone: None,
            skip_body_pipeline: false,
            head_ik_eye_height: None,
            neck_bone: Some("J_Bip_C_Neck".to_string()),
            model_root_id: None,
            model_root_local_y: 0.0,
            body_to_head_offset: [0.0, 0.0, 0.0],
            neck_bone_id: None,
            neck_rest_translation: None,
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

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn to_mms_ast(&self, _world: &crate::engine::ecs::World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut c = ce("AvatarControl")
            .with_call("head_bone", vec![s(&self.head_bone)])
            .with_call("body_yaw_threshold", vec![num(self.body_yaw_threshold as f64)])
            .with_call("body_yaw_rate", vec![num(self.body_yaw_rate as f64)]);
        if let Some(b) = &self.left_hand_bone {
            c = c.with_call("left_hand_bone", vec![s(b)]);
        }
        if let Some(b) = &self.right_hand_bone {
            c = c.with_call("right_hand_bone", vec![s(b)]);
        }
        if self.forward_plus_z {
            c = c.with_call("forward_plus_z", vec![]);
        }
        if let Some(factor) = self.hand_rotation_smoothing {
            c = c.with_call("hand_rotation_smoothing", vec![num(factor as f64)]);
        }
        if let Some(b) = &self.camera_bone {
            c = c.with_call("camera_bone", vec![s(b)]);
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
        if self.body_to_head_offset != [0.0, 0.0, 0.0] {
            c = c.with_call(
                "body_to_head_offset",
                vec![array(vec![
                    num(self.body_to_head_offset[0] as f64),
                    num(self.body_to_head_offset[1] as f64),
                    num(self.body_to_head_offset[2] as f64),
                ])],
            );
        }
        if let Some(b) = &self.hips_bone {
            c = c.with_call("hips_bone", vec![s(b)]);
        }
        c
    }
}
