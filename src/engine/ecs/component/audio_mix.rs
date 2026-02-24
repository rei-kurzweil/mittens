use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone)]
pub struct AudioMixComponent {
    pub weights: Vec<f32>,
}

impl AudioMixComponent {
    pub fn new(weights: Vec<f32>) -> Self {
        Self { weights }
    }

    pub fn weight_for_branch(&self, branch_index: usize) -> f32 {
        self.weights.get(branch_index).copied().unwrap_or(1.0)
    }
}

impl Default for AudioMixComponent {
    fn default() -> Self {
        Self { weights: vec![] }
    }
}

impl Component for AudioMixComponent {
    fn name(&self) -> &'static str {
        "audio_mix"
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
        map.insert("weights".to_string(), serde_json::json!(self.weights));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("weights") {
            self.weights = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode weights: {e}"))?;
        }
        Ok(())
    }
}
