use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy)]
pub struct KeyframeComponent {
    /// When this keyframe should fire, in beats.
    pub beat: f64,

    component: Option<ComponentId>,
}

impl KeyframeComponent {
    pub fn new(beat: f64) -> Self {
        Self {
            beat,
            component: None,
        }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Component for KeyframeComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "keyframe"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push(
            component,
            crate::engine::ecs::SignalValue::RegisterKeyframe { component },
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("beat".to_string(), serde_json::json!(self.beat));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(beat) = data.get("beat") {
            self.beat = serde_json::from_value(beat.clone())
                .map_err(|e| format!("Failed to decode beat: {}", e))?;
        }
        Ok(())
    }
}
