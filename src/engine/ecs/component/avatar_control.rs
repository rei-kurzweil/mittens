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
///               │                 neck_parent
///               │                   └── splice_head  ← injected by system
///               │                         └── J_Bip_C_Neck (displaced)
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
    /// Name of the bone to displace for head rotation. Default: "J_Bip_C_Neck".
    pub head_bone: String,

    /// Name of the left hand bone to splice. `None` = no left hand splice.
    pub left_hand_bone: Option<String>,

    /// Name of the right hand bone to splice. `None` = no right hand splice.
    pub right_hand_bone: Option<String>,

    /// Yaw delta (radians) that triggers body rotation. Default: π/4 (45°).
    pub body_yaw_threshold: f32,

    /// Body rotation rate (radians/sec). Default: 3.0.
    pub body_yaw_rate: f32,

    /// Use +Z as the forward axis (desktop). Default false = -Z (OpenXR).
    pub forward_plus_z: bool,

    /// Current world-space body yaw (radians). Maintained by AvatarControlSystem.
    pub(crate) body_yaw: f32,

    // Runtime IDs set by AvatarControlSystem on first tick:
    /// model_root's local translation at init time, used as the intended world-space
    /// Y offset each tick. Stored so pitch of driven_t can be compensated.
    pub(crate) model_root_rest_local: [f32; 3],
    pub(crate) splice_head:          Option<ComponentId>,
    pub(crate) displaced_head:       Option<ComponentId>,
    /// Immediate parent of the displaced left hand bone (controller's driven_t or plain TC).
    pub(crate) splice_left_hand:     Option<ComponentId>,
    pub(crate) displaced_left_hand:  Option<ComponentId>,
    /// Immediate parent of the displaced right hand bone.
    pub(crate) splice_right_hand:    Option<ComponentId>,
    pub(crate) displaced_right_hand: Option<ComponentId>,

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

    pub fn with_body_yaw_threshold(mut self, t: f32) -> Self {
        self.body_yaw_threshold = t;
        self
    }

    pub fn with_body_yaw_rate(mut self, r: f32) -> Self {
        self.body_yaw_rate = r;
        self
    }

    /// Override the starting body yaw (radians).
    /// Use `std::f32::consts::PI` for VR setups where the model faces -Z at rest.
    /// Default: 0.0 (model faces +Z, standard for `forward_plus_z` desktop setups).
    pub fn with_initial_yaw(mut self, yaw: f32) -> Self {
        self.body_yaw = yaw;
        self
    }

    /// Use +Z as the forward axis. Required for desktop setups using
    /// `InputTransformModeComponent::forward_z()`.
    pub fn with_forward_plus_z(mut self) -> Self {
        self.forward_plus_z = true;
        self
    }
}

impl Default for AvatarControlComponent {
    fn default() -> Self {
        Self {
            head_bone: "J_Bip_C_Neck".to_string(),
            left_hand_bone: None,
            right_hand_bone: None,
            body_yaw_threshold: std::f32::consts::FRAC_PI_4,
            body_yaw_rate: 3.0,
            forward_plus_z: false,
            body_yaw: 0.0,
            model_root_rest_local: [0.0, 0.0, 0.0],
            splice_head: None,
            displaced_head: None,
            splice_left_hand: None,
            displaced_left_hand: None,
            splice_right_hand: None,
            displaced_right_hand: None,
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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("head_bone".to_string(), serde_json::json!(self.head_bone));
        if let Some(ref b) = self.left_hand_bone {
            map.insert("left_hand_bone".to_string(), serde_json::json!(b));
        }
        if let Some(ref b) = self.right_hand_bone {
            map.insert("right_hand_bone".to_string(), serde_json::json!(b));
        }
        map.insert("body_yaw_threshold".to_string(), serde_json::json!(self.body_yaw_threshold));
        map.insert("body_yaw_rate".to_string(), serde_json::json!(self.body_yaw_rate));
        map.insert("forward_plus_z".to_string(), serde_json::json!(self.forward_plus_z));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("head_bone") {
            if let Some(s) = v.as_str() { self.head_bone = s.to_string(); }
        }
        if let Some(v) = data.get("left_hand_bone") {
            self.left_hand_bone = v.as_str().map(|s| s.to_string());
        }
        if let Some(v) = data.get("right_hand_bone") {
            self.right_hand_bone = v.as_str().map(|s| s.to_string());
        }
        if let Some(v) = data.get("body_yaw_threshold") {
            if let Some(f) = v.as_f64() { self.body_yaw_threshold = f as f32; }
        }
        if let Some(v) = data.get("body_yaw_rate") {
            if let Some(f) = v.as_f64() { self.body_yaw_rate = f as f32; }
        }
        if let Some(v) = data.get("forward_plus_z") {
            if let Some(b) = v.as_bool() { self.forward_plus_z = b; }
        }
        Ok(())
    }
}
