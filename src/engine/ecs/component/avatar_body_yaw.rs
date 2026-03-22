use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Sits between the avatar body pipeline output and `model_root`.
/// Tracks head yaw and smoothly rotates the body to follow when the
/// relative yaw exceeds `threshold`.
///
/// Topology:
/// ```text
/// TransformPipelineOutput (av_output)
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
}

impl Default for AvatarBodyYawComponent {
    fn default() -> Self {
        Self {
            threshold: std::f32::consts::FRAC_PI_4,
            rate: 3.0,
            hmd_driven_transform: None,
            body_yaw: std::f32::consts::PI,
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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("threshold".to_string(), serde_json::json!(self.threshold));
        map.insert("rate".to_string(), serde_json::json!(self.rate));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("threshold") {
            if let Some(f) = v.as_f64() {
                self.threshold = f as f32;
            }
        }
        if let Some(v) = data.get("rate") {
            if let Some(f) = v.as_f64() {
                self.rate = f as f32;
            }
        }
        Ok(())
    }
}
