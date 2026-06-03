use crate::engine::ecs::component::Component;
use crate::engine::ecs::ComponentId;

/// Sits between the avatar body pipeline root and `model_root`.
/// Tracks head yaw and smoothly rotates the body to follow when the
/// relative yaw exceeds `threshold`.
///
/// Topology:
/// ```text
/// TransformForkTRS (body_pipeline)
///   AvatarBodyYawComponent   ← this node
///     TransformComponent (model_root, Y-offset + base rotation)
///       GLTFComponent
/// ```
#[derive(Debug, Clone)]
pub struct AvatarBodyYawComponent {
    /// Yaw delta (radians) that triggers body rotation. Default: π/4 (45°).
    pub threshold: f32,

    /// Body rotation rate (radians/sec). Default: 3.0 rad/s.
    pub rate: f32,

    /// ComponentId of the HMD-driven TransformComponent to read yaw from
    /// (`avatar_driven_t` in vr-input). Wired at scene construction.
    pub hmd_driven_transform: Option<ComponentId>,

    /// Current world-space body yaw (radians). Initialized to π to match the
    /// model_root's initial `rotation_euler(0, π, 0)` base flip.
    pub(crate) body_yaw: f32,

    /// When true, use +Z as the forward axis when extracting yaw from the driven
    /// transform's world matrix. Use for desktop/keyboard setups with
    /// `InputTransformModeComponent::forward_z()`. Default false = -Z (OpenXR).
    pub forward_plus_z: bool,

    component: Option<ComponentId>,
}

impl AvatarBodyYawComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, t: f32) -> Self {
        self.threshold = t;
        self
    }

    pub fn with_rate(mut self, r: f32) -> Self {
        self.rate = r;
        self
    }

    pub fn with_hmd_driven_transform(mut self, id: ComponentId) -> Self {
        self.hmd_driven_transform = Some(id);
        self
    }

    /// Override the starting body yaw (radians). Defaults to π to match the vr-input
    /// model_root base flip. Set to 0.0 for setups with no base rotation on model_root.
    pub fn with_initial_yaw(mut self, yaw: f32) -> Self {
        self.body_yaw = yaw;
        self
    }

    /// Use +Z as the forward axis for yaw extraction. Required for desktop setups
    /// using `InputTransformModeComponent::forward_z()`.
    pub fn with_forward_plus_z(mut self) -> Self {
        self.forward_plus_z = true;
        self
    }
}

impl Default for AvatarBodyYawComponent {
    fn default() -> Self {
        Self {
            threshold: std::f32::consts::FRAC_PI_4,
            rate: 3.0,
            hmd_driven_transform: None,
            body_yaw: std::f32::consts::PI,
            forward_plus_z: false,
            component: None,
        }
    }
}

impl Component for AvatarBodyYawComponent {
    fn name(&self) -> &'static str {
        "avatar_body_yaw"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
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
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut c = ce("AvatarBodyYaw")
            .with_call("threshold", vec![num(self.threshold as f64)])
            .with_call("rate", vec![num(self.rate as f64)]);
        if self.forward_plus_z {
            c = c.with_call("forward_plus_z", vec![]);
        }
        c
    }
}
