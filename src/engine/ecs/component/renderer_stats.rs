use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::CameraTarget;

#[derive(Debug, Clone)]
pub struct RendererStatsComponent {
    pub enabled: bool,

    /// Which renderer timing source this stats widget displays.
    ///
    /// This is explicit (no scene traversal): set it to `CameraTarget::Window` or
    /// `CameraTarget::Xr`.
    pub target: CameraTarget,

    /// Minimum time between text updates.
    ///
    /// Updating text is relatively expensive because it rebuilds glyph subtrees.
    pub update_interval_sec: f32,

    /// Exponential moving-average smoothing factor in $[0, 1]$.
    ///
    /// - 0.0: no smoothing
    /// - 0.9: heavy smoothing (default)
    pub smoothing: f32,

    /// Text color (RGBA).
    pub color: [f32; 4],

    /// Whether spawned glyphs should be emissive (unlit).
    pub emissive: bool,

    // --- runtime-only state (not serialized) ---
    component_id: Option<ComponentId>,
    time_since_update_sec: f32,
    smoothed_fps: Option<f32>,

    // Auto-managed subtree ids.
    text: Option<ComponentId>,
    text_color: Option<ComponentId>,
    text_emissive: Option<ComponentId>,
}

impl RendererStatsComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            target: CameraTarget::Window,
            update_interval_sec: 0.25,
            smoothing: 0.9,
            color: [1.0, 1.0, 1.0, 1.0],
            emissive: true,

            component_id: None,
            time_since_update_sec: 0.0,
            smoothed_fps: None,

            text: None,
            text_color: None,
            text_emissive: None,
        }
    }

    pub fn with_camera_target(mut self, target: CameraTarget) -> Self {
        self.target = target;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component_id
    }

    pub fn accumulate_time(&mut self, dt_sec: f32) {
        if !self.enabled {
            return;
        }
        self.time_since_update_sec = (self.time_since_update_sec + dt_sec).max(0.0);
    }

    pub fn should_update(&self) -> bool {
        if !self.enabled {
            return false;
        }
        self.time_since_update_sec >= self.update_interval_sec.max(0.0)
    }

    pub fn reset_update_timer(&mut self) {
        self.time_since_update_sec = 0.0;
    }

    pub fn smooth_fps(&mut self, fps: f32) -> f32 {
        let fps = if fps.is_finite() { fps.max(0.0) } else { 0.0 };
        let s = self.smoothing.clamp(0.0, 1.0);
        let out = match self.smoothed_fps {
            None => fps,
            Some(prev) => prev * s + fps * (1.0 - s),
        };
        self.smoothed_fps = Some(out);
        out
    }

    pub(crate) fn runtime_subtree_ids_mut(
        &mut self,
    ) -> (
        &mut Option<ComponentId>,
        &mut Option<ComponentId>,
        &mut Option<ComponentId>,
    ) {
        (
            &mut self.text,
            &mut self.text_color,
            &mut self.text_emissive,
        )
    }
}

impl Default for RendererStatsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for RendererStatsComponent {
    fn name(&self) -> &'static str {
        "renderer_stats"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component_id = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::json!(self.enabled));
        map.insert(
            "target".to_string(),
            serde_json::json!(match self.target {
                CameraTarget::Window => "window",
                CameraTarget::Xr => "xr",
            }),
        );
        map.insert(
            "update_interval_sec".to_string(),
            serde_json::json!(self.update_interval_sec),
        );
        map.insert("smoothing".to_string(), serde_json::json!(self.smoothing));
        map.insert("color".to_string(), serde_json::json!(self.color));
        map.insert("emissive".to_string(), serde_json::json!(self.emissive));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode enabled: {e}"))?;
        }
        self.target = match data
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required field: target".to_string())?
        {
            "xr" => CameraTarget::Xr,
            "window" => CameraTarget::Window,
            other => return Err(format!("Invalid target: {other}")),
        };
        if let Some(update_interval_sec) = data.get("update_interval_sec") {
            self.update_interval_sec = serde_json::from_value(update_interval_sec.clone())
                .map_err(|e| format!("Failed to decode update_interval_sec: {e}"))?;
        }
        if let Some(smoothing) = data.get("smoothing") {
            self.smoothing = serde_json::from_value(smoothing.clone())
                .map_err(|e| format!("Failed to decode smoothing: {e}"))?;
        }
        if let Some(color) = data.get("color") {
            self.color = serde_json::from_value(color.clone())
                .map_err(|e| format!("Failed to decode color: {e}"))?;
        }
        if let Some(emissive) = data.get("emissive") {
            self.emissive = serde_json::from_value(emissive.clone())
                .map_err(|e| format!("Failed to decode emissive: {e}"))?;
        }

        // Sanitize.
        if !self.update_interval_sec.is_finite() || self.update_interval_sec < 0.0 {
            self.update_interval_sec = 0.25;
        }
        if !self.smoothing.is_finite() {
            self.smoothing = 0.9;
        }
        Ok(())
    }
}
