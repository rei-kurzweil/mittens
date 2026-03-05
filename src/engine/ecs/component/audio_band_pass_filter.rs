use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct AudioBandPassFilterComponent {
    /// Center frequency of the band-pass filter.
    pub center_hz: f32,
    /// Bandwidth in octaves.
    ///
    /// Internally we derive:
    /// - low_cutoff = center / 2^(bw/2)
    /// - high_cutoff = center * 2^(bw/2)
    pub bandwidth_octaves: f32,
    pub resonance: f32,
}

impl AudioBandPassFilterComponent {
    pub fn new(center_hz: f32, bandwidth_octaves: f32, resonance: f32) -> Self {
        Self {
            center_hz,
            bandwidth_octaves,
            resonance,
        }
    }
}

impl Default for AudioBandPassFilterComponent {
    fn default() -> Self {
        Self {
            center_hz: 600.0,
            bandwidth_octaves: 1.0,
            resonance: 0.2,
        }
    }
}

impl Component for AudioBandPassFilterComponent {
    fn name(&self) -> &'static str {
        "audio_band_pass_filter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push(
            component,
            crate::engine::ecs::SignalValue::AudioGraphDirtyImmediate { component },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("center_hz".to_string(), serde_json::json!(self.center_hz));
        map.insert(
            "bandwidth_octaves".to_string(),
            serde_json::json!(self.bandwidth_octaves),
        );
        map.insert("resonance".to_string(), serde_json::json!(self.resonance));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("center_hz") {
            self.center_hz = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode center_hz: {e}"))?;
        }
        if let Some(v) = data.get("bandwidth_octaves") {
            self.bandwidth_octaves = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode bandwidth_octaves: {e}"))?;
        }
        if let Some(v) = data.get("resonance") {
            self.resonance = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode resonance: {e}"))?;
        }
        Ok(())
    }
}
