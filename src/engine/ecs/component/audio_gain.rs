use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct AudioGainComponent {
    pub gain: f32,
}

impl AudioGainComponent {
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }
}

impl Default for AudioGainComponent {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl Component for AudioGainComponent {
    fn name(&self) -> &'static str {
        "audio_gain"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::AudioGraphDirtyImmediate {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("gain".to_string(), serde_json::json!(self.gain));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("gain") {
            self.gain = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode gain: {e}"))?;
        }
        Ok(())
    }
}
