use super::Component;

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
