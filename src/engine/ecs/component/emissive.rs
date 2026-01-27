use crate::engine::ecs::component::Component;
use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, Default)]
pub struct EmissiveComponent {
    pub enabled: bool,
}

impl EmissiveComponent {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn on() -> Self {
        Self { enabled: true }
    }

    pub fn off() -> Self {
        Self { enabled: false }
    }
}

impl Component for EmissiveComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "emissive"
    }

    fn init(&mut self, queue: &mut CommandQueue, component: ComponentId) {
        queue.queue_register_emissive(component);
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enabled".to_string(), serde_json::Value::Bool(self.enabled));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("enabled") {
            self.enabled = v.as_bool().unwrap_or(false);
        }
        Ok(())
    }
}
