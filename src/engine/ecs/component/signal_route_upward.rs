use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Pipeline operator: route matching intents upward to the first ancestor whose type matches.
#[derive(Debug, Clone, Default)]
pub struct SignalRouteUpwardComponent {
    pub intent_kind: String,
    pub parent_type: String,
}

impl SignalRouteUpwardComponent {
    pub fn new(intent_kind: impl Into<String>, parent_type: impl Into<String>) -> Self {
        Self {
            intent_kind: intent_kind.into(),
            parent_type: parent_type.into(),
        }
    }
}

impl Component for SignalRouteUpwardComponent {
    fn name(&self) -> &'static str {
        "signal_route_upward"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterSignalRouteUpward {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RemoveSignalRouteUpward {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "intent_kind".to_string(),
            serde_json::json!(self.intent_kind),
        );
        map.insert(
            "parent_type".to_string(),
            serde_json::json!(self.parent_type),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let Some(v) = data.get("intent_kind") else {
            return Err("Missing intent_kind".to_string());
        };
        let Some(p) = data.get("parent_type") else {
            return Err("Missing parent_type".to_string());
        };

        self.intent_kind = serde_json::from_value(v.clone())
            .map_err(|e| format!("Failed to decode intent_kind: {e}"))?;
        self.parent_type = serde_json::from_value(p.clone())
            .map_err(|e| format!("Failed to decode parent_type: {e}"))?;
        Ok(())
    }
}
