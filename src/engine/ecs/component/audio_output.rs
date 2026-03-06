use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy)]
pub struct AudioOutputComponent {
    /// Placeholder: when present, `AudioSystem` will try to start the default output stream.
    pub enabled: bool,

    component: Option<ComponentId>,
}

impl AudioOutputComponent {
    pub fn new() -> Self {
        Self {
            enabled: true,
            component: None,
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            component: None,
        }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for AudioOutputComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudioOutputComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "audio_output"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        if self.enabled {
            emit.push_intent_now(
                component,
                crate::engine::ecs::IntentValue::RegisterAudioOutput { component },
            );
            emit.push_intent_now(
                component,
                crate::engine::ecs::IntentValue::AudioGraphDirtyImmediate { component },
            );
        }
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
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(enabled) = data.get("enabled") {
            self.enabled = serde_json::from_value(enabled.clone())
                .map_err(|e| format!("Failed to decode enabled: {}", e))?;
        }
        Ok(())
    }
}
