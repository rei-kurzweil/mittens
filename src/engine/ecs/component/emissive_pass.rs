use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Default)]
pub struct EmissivePassComponent;

impl EmissivePassComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for EmissivePassComponent {
    fn name(&self) -> &'static str {
        "emissive_pass"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        std::collections::HashMap::new()
    }

    fn decode(
        &mut self,
        _data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        Ok(())
    }
}