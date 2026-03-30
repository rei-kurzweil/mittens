use crate::engine::ecs::component::Component;

#[derive(Debug, Clone)]
pub struct BloomComponent {
    pub enabled: bool,
    pub intensity: f32,
    pub radius_ndc: f32,
    pub emissive_scale: f32,
    pub half_res: bool,
    pub debug_show_emissive: bool,
    pub debug_show_bloom: bool,
    pub debug_emissive_texture: Option<String>,
    pub debug_bloom_texture: Option<String>,
}

impl Default for BloomComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl BloomComponent {
    pub fn new() -> Self {
        let cfg = crate::engine::graphics::BloomConfig::default();
        Self {
            enabled: true,
            intensity: cfg.intensity,
            radius_ndc: cfg.radius_ndc,
            emissive_scale: cfg.emissive_scale,
            half_res: cfg.half_res,
            debug_show_emissive: false,
            debug_show_bloom: false,
            debug_emissive_texture: None,
            debug_bloom_texture: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        if intensity.is_finite() {
            self.intensity = intensity.max(0.0);
        }
        self
    }

    pub fn with_radius_ndc(mut self, radius_ndc: f32) -> Self {
        if radius_ndc.is_finite() {
            self.radius_ndc = radius_ndc.max(0.0);
        }
        self
    }

    pub fn with_emissive_scale(mut self, emissive_scale: f32) -> Self {
        if emissive_scale.is_finite() {
            self.emissive_scale = emissive_scale.max(0.0);
        }
        self
    }

    pub fn with_half_res(mut self, half_res: bool) -> Self {
        self.half_res = half_res;
        self
    }

    pub fn with_debug_show_emissive(mut self, debug_show_emissive: bool) -> Self {
        self.debug_show_emissive = debug_show_emissive;
        self
    }

    pub fn with_debug_show_bloom(mut self, debug_show_bloom: bool) -> Self {
        self.debug_show_bloom = debug_show_bloom;
        self
    }

    pub fn with_debug_emissive_texture(
        mut self,
        debug_emissive_texture: impl Into<String>,
    ) -> Self {
        self.debug_emissive_texture = Some(debug_emissive_texture.into());
        self
    }

    pub fn with_debug_bloom_texture(mut self, debug_bloom_texture: impl Into<String>) -> Self {
        self.debug_bloom_texture = Some(debug_bloom_texture.into());
        self
    }
}

impl Component for BloomComponent {
    fn name(&self) -> &'static str {
        "bloom"
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
        map.insert("intensity".to_string(), serde_json::json!(self.intensity));
        map.insert("radius_ndc".to_string(), serde_json::json!(self.radius_ndc));
        map.insert(
            "emissive_scale".to_string(),
            serde_json::json!(self.emissive_scale),
        );
        map.insert("half_res".to_string(), serde_json::json!(self.half_res));
        map.insert(
            "debug_show_emissive".to_string(),
            serde_json::json!(self.debug_show_emissive),
        );
        map.insert(
            "debug_show_bloom".to_string(),
            serde_json::json!(self.debug_show_bloom),
        );
        if let Some(debug_emissive_texture) = &self.debug_emissive_texture {
            map.insert(
                "debug_emissive_texture".to_string(),
                serde_json::json!(debug_emissive_texture),
            );
        }
        if let Some(debug_bloom_texture) = &self.debug_bloom_texture {
            map.insert(
                "debug_bloom_texture".to_string(),
                serde_json::json!(debug_bloom_texture),
            );
        }
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode bloom.enabled: {e}"))?;
        }
        if let Some(intensity) = data.get("intensity") {
            self.intensity = serde_json::from_value::<f32>(intensity.clone())
                .map_err(|e| format!("Failed to decode bloom.intensity: {e}"))?
                .max(0.0);
        }
        if let Some(radius_ndc) = data.get("radius_ndc") {
            self.radius_ndc = serde_json::from_value::<f32>(radius_ndc.clone())
                .map_err(|e| format!("Failed to decode bloom.radius_ndc: {e}"))?
                .max(0.0);
        }
        if let Some(emissive_scale) = data.get("emissive_scale") {
            self.emissive_scale = serde_json::from_value::<f32>(emissive_scale.clone())
                .map_err(|e| format!("Failed to decode bloom.emissive_scale: {e}"))?
                .max(0.0);
        }
        if let Some(half_res) = data.get("half_res") {
            self.half_res = serde_json::from_value(half_res.clone())
                .map_err(|e| format!("Failed to decode bloom.half_res: {e}"))?;
        }
        if let Some(debug_show_emissive) = data.get("debug_show_emissive") {
            self.debug_show_emissive = serde_json::from_value(debug_show_emissive.clone())
                .map_err(|e| format!("Failed to decode bloom.debug_show_emissive: {e}"))?;
        }
        if let Some(debug_show_bloom) = data.get("debug_show_bloom") {
            self.debug_show_bloom = serde_json::from_value(debug_show_bloom.clone())
                .map_err(|e| format!("Failed to decode bloom.debug_show_bloom: {e}"))?;
        }
        if let Some(debug_emissive_texture) = data.get("debug_emissive_texture") {
            self.debug_emissive_texture = Some(
                serde_json::from_value(debug_emissive_texture.clone())
                    .map_err(|e| format!("Failed to decode bloom.debug_emissive_texture: {e}"))?,
            );
        }
        if let Some(debug_bloom_texture) = data.get("debug_bloom_texture") {
            self.debug_bloom_texture = Some(
                serde_json::from_value(debug_bloom_texture.clone())
                    .map_err(|e| format!("Failed to decode bloom.debug_bloom_texture: {e}"))?,
            );
        }
        Ok(())
    }
}