use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct AudioLowPassFilterComponent {
    pub cutoff_hz: f32,
    pub resonance: f32,
}

impl AudioLowPassFilterComponent {
    pub fn new(cutoff_hz: f32, resonance: f32) -> Self {
        Self {
            cutoff_hz,
            resonance,
        }
    }
}

impl Default for AudioLowPassFilterComponent {
    fn default() -> Self {
        Self {
            cutoff_hz: 2000.0,
            resonance: 0.2,
        }
    }
}

impl Component for AudioLowPassFilterComponent {
    fn name(&self) -> &'static str {
        "audio_low_pass_filter"
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
        map.insert("cutoff_hz".to_string(), serde_json::json!(self.cutoff_hz));
        map.insert("resonance".to_string(), serde_json::json!(self.resonance));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("cutoff_hz") {
            self.cutoff_hz = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode cutoff_hz: {e}"))?;
        }
        if let Some(v) = data.get("resonance") {
            self.resonance = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode resonance: {e}"))?;
        }
        Ok(())
    }
}
