use crate::engine::ecs::component::Component;

#[derive(Debug, Clone)]
pub struct BlurPassComponent {
    pub enabled: bool,
    pub radius_ndc: f32,
    pub half_res: bool,
}

impl Default for BlurPassComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl BlurPassComponent {
    pub fn new() -> Self {
        let cfg = crate::engine::graphics::BlurPassConfig::default();
        Self {
            enabled: true,
            radius_ndc: cfg.radius_ndc,
            half_res: cfg.half_res,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_radius_ndc(mut self, radius_ndc: f32) -> Self {
        if radius_ndc.is_finite() {
            self.radius_ndc = radius_ndc.max(0.0);
        }
        self
    }

    pub fn with_half_res(mut self, half_res: bool) -> Self {
        self.half_res = half_res;
        self
    }
}

impl Component for BlurPassComponent {
    fn name(&self) -> &'static str {
        "blur_pass"
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
        map.insert("radius_ndc".to_string(), serde_json::json!(self.radius_ndc));
        map.insert("half_res".to_string(), serde_json::json!(self.half_res));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode blur_pass.enabled: {e}"))?;
        }
        if let Some(radius_ndc) = data.get("radius_ndc") {
            self.radius_ndc = serde_json::from_value::<f32>(radius_ndc.clone())
                .map_err(|e| format!("Failed to decode blur_pass.radius_ndc: {e}"))?
                .max(0.0);
        }
        if let Some(half_res) = data.get("half_res") {
            self.half_res = serde_json::from_value(half_res.clone())
                .map_err(|e| format!("Failed to decode blur_pass.half_res: {e}"))?;
        }
        Ok(())
    }
}
