use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy)]
pub struct AudioBufferSizeComponent {
    /// Requested output buffer size in frames.
    ///
    /// If present alongside an `AudioOutputComponent`, the most recently
    /// registered buffer size wins.
    pub frames: u32,

    component: Option<ComponentId>,
}

impl AudioBufferSizeComponent {
    pub fn new(frames: u32) -> Self {
        Self {
            frames,
            component: None,
        }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for AudioBufferSizeComponent {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Component for AudioBufferSizeComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "audio_buffer_size"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        if self.frames > 0 {
            emit.push(
                component,
                crate::engine::ecs::SignalValue::RegisterAudioBufferSize { component },
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
        map.insert("frames".to_string(), serde_json::json!(self.frames));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(frames) = data.get("frames") {
            self.frames = serde_json::from_value(frames.clone())
                .map_err(|e| format!("Failed to decode frames: {}", e))?;
        }
        Ok(())
    }
}
