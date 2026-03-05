use super::Component;
use crate::engine::ecs::ComponentId;

/// Controls the global beat clock tempo (BPM).
///
/// Intended to be singleton-like: the most recently registered ClockComponent wins.
#[derive(Debug, Clone, Copy)]
pub struct ClockComponent {
    pub bpm: f64,

    component: Option<ComponentId>,
}

impl ClockComponent {
    pub fn new() -> Self {
        Self {
            bpm: 120.0,
            component: None,
        }
    }

    pub fn with_bpm(mut self, bpm: f64) -> Self {
        self.bpm = bpm;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for ClockComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ClockComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "clock"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push(
            component,
            crate::engine::ecs::SignalValue::RegisterClock { component },
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
        map.insert("bpm".to_string(), serde_json::json!(self.bpm));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(bpm) = data.get("bpm") {
            self.bpm = serde_json::from_value(bpm.clone())
                .map_err(|e| format!("Failed to decode bpm: {}", e))?;
        }
        Ok(())
    }
}
