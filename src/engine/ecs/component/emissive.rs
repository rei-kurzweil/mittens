use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct EmissiveComponent {
    pub intensity: f32,
}

impl Default for EmissiveComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl EmissiveComponent {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: if intensity.is_finite() {
                intensity.max(0.0)
            } else {
                0.0
            },
        }
    }

    pub fn on() -> Self {
        Self { intensity: 1.0 }
    }

    pub fn off() -> Self {
        Self { intensity: 0.0 }
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEmissive {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("intensity".to_string(), serde_json::json!(self.intensity));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let Some(v) = data.get("intensity") else {
            return Err("emissive.intensity missing".to_string());
        };
        self.intensity = serde_json::from_value::<f32>(v.clone())
            .map_err(|e| format!("Failed to decode emissive.intensity: {e}"))?
            .max(0.0);
        Ok(())
    }
}
