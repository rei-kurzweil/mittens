use super::Component;

/// Strips pitch and roll from the rotation channel, keeping only the Y-axis component.
/// Equivalent to projecting the input quaternion onto the Y-rotation subspace:
/// `normalize([0, q.y, 0, q.w])`.
///
/// Convention-independent: the output is always a pure-Y quaternion whose angle
/// equals the Y-rotation of the input quaternion.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuatExtractYawComponent;

impl QuatExtractYawComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for QuatExtractYawComponent {
    fn name(&self) -> &'static str {
        "quat_extract_yaw"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Stateful body-yaw follow: extracts world-Y yaw from the input rotation, then
/// advances a running `body_yaw` toward the head yaw when the delta exceeds
/// `threshold`, at `rate` rad/s. Outputs a pure-Y quaternion for `body_yaw`.
///
/// State lives in `TransformPipelineSystem` alongside `QuatTemporalFilter` state,
/// keyed by stage path.
///
/// `forward_plus_z` controls the yaw-extraction convention:
/// - `false` (default, OpenXR/VR): -Z forward; at identity rotation yaw = π.
/// - `true` (desktop): +Z forward; at identity rotation yaw = 0.
///
/// `initial_yaw` seeds `body_yaw` on the first frame. Set to `π` for VR so the
/// body starts aligned with OpenXR's -Z-forward rest pose.
#[derive(Debug, Clone, Copy)]
pub struct QuatYawFollowComponent {
    pub threshold: f32,
    pub rate: f32,
    pub forward_plus_z: bool,
    pub initial_yaw: f32,
}

impl QuatYawFollowComponent {
    pub fn new(threshold: f32, rate: f32) -> Self {
        Self {
            threshold,
            rate,
            forward_plus_z: false,
            initial_yaw: 0.0,
        }
    }

    pub fn with_forward_plus_z(mut self) -> Self {
        self.forward_plus_z = true;
        self
    }

    pub fn with_forward_plus_z_if(mut self, value: bool) -> Self {
        self.forward_plus_z = value;
        self
    }

    pub fn with_initial_yaw(mut self, yaw: f32) -> Self {
        self.initial_yaw = yaw;
        self
    }
}

impl Default for QuatYawFollowComponent {
    fn default() -> Self {
        Self::new(std::f32::consts::FRAC_PI_4, 3.0)
    }
}

impl Component for QuatYawFollowComponent {
    fn name(&self) -> &'static str {
        "quat_yaw_follow"
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
        map.insert("forward_plus_z".to_string(), serde_json::json!(self.forward_plus_z));
        map.insert("initial_yaw".to_string(), serde_json::json!(self.initial_yaw));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("threshold") {
            if let Some(f) = v.as_f64() { self.threshold = f as f32; }
        }
        if let Some(v) = data.get("rate") {
            if let Some(f) = v.as_f64() { self.rate = f as f32; }
        }
        if let Some(v) = data.get("forward_plus_z") {
            if let Some(b) = v.as_bool() { self.forward_plus_z = b; }
        }
        if let Some(v) = data.get("initial_yaw") {
            if let Some(f) = v.as_f64() { self.initial_yaw = f as f32; }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vector3TemporalFilterComponent {
    pub smoothing_factor: f32,
}

impl Vector3TemporalFilterComponent {
    pub fn new() -> Self {
        Self {
            smoothing_factor: 1.0,
        }
    }

    pub fn with_smoothing_factor(mut self, smoothing_factor: f32) -> Self {
        self.smoothing_factor = smoothing_factor;
        self
    }
}

impl Default for Vector3TemporalFilterComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Vector3TemporalFilterComponent {
    fn name(&self) -> &'static str {
        "vector3_temporal_filter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "smoothing_factor".to_string(),
            serde_json::json!(self.smoothing_factor),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(value) = data.get("smoothing_factor") {
            self.smoothing_factor = serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to decode smoothing_factor: {e}"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QuatTemporalFilterComponent {
    pub smoothing_factor: f32,
}

impl QuatTemporalFilterComponent {
    pub fn new() -> Self {
        Self {
            smoothing_factor: 1.0,
        }
    }

    pub fn with_smoothing_factor(mut self, smoothing_factor: f32) -> Self {
        self.smoothing_factor = smoothing_factor;
        self
    }
}

impl Default for QuatTemporalFilterComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QuatTemporalFilterComponent {
    fn name(&self) -> &'static str {
        "quat_temporal_filter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "smoothing_factor".to_string(),
            serde_json::json!(self.smoothing_factor),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(value) = data.get("smoothing_factor") {
            self.smoothing_factor = serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to decode smoothing_factor: {e}"))?;
        }
        Ok(())
    }
}
