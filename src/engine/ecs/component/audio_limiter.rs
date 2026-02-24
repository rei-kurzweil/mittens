use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct AudioLimiterComponent {
    pub attack_ms: f32,
    pub release_ms: f32,
    pub threshold: f32,
}

impl AudioLimiterComponent {
    pub fn new(attack_ms: f32, release_ms: f32, threshold: f32) -> Self {
        Self {
            attack_ms,
            release_ms,
            threshold,
        }
    }
}

impl Default for AudioLimiterComponent {
    fn default() -> Self {
        Self {
            attack_ms: 5.0,
            release_ms: 50.0,
            threshold: 0.9,
        }
    }
}

impl Component for AudioLimiterComponent {
    fn name(&self) -> &'static str {
        "audio_limiter"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, queue: &mut crate::engine::ecs::CommandQueue, component: ComponentId) {
        queue.queue_audio_graph_dirty(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("attack_ms".to_string(), serde_json::json!(self.attack_ms));
        map.insert("release_ms".to_string(), serde_json::json!(self.release_ms));
        map.insert("threshold".to_string(), serde_json::json!(self.threshold));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("attack_ms") {
            self.attack_ms = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode attack_ms: {e}"))?;
        }
        if let Some(v) = data.get("release_ms") {
            self.release_ms = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode release_ms: {e}"))?;
        }
        if let Some(v) = data.get("threshold") {
            self.threshold = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode threshold: {e}"))?;
        }
        Ok(())
    }
}
