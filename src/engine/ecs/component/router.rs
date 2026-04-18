use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

#[derive(Debug, Clone, Default)]
pub struct RouterComponent {
    pub target_name: Option<String>,
    pub ignore_names: Vec<String>,
    component: Option<ComponentId>,
}

impl RouterComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_target_name(mut self, target_name: impl Into<String>) -> Self {
        self.target_name = Some(target_name.into());
        self
    }

    pub fn with_ignored_names<I, S>(mut self, ignore_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.ignore_names = ignore_names.into_iter().map(Into::into).collect();
        self
    }
}

impl Component for RouterComponent {
    fn name(&self) -> &'static str {
        "router"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
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
            IntentValue::RegisterRouter {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        if let Some(target_name) = &self.target_name {
            map.insert("target_name".to_string(), serde_json::json!(target_name));
        }
        map.insert("ignore_names".to_string(), serde_json::json!(self.ignore_names));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        self.target_name = match data.get("target_name") {
            Some(value) => Some(
                serde_json::from_value(value.clone())
                    .map_err(|e| format!("Failed to decode target_name: {e}"))?,
            ),
            None => None,
        };

        self.ignore_names = match data.get("ignore_names") {
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to decode ignore_names: {e}"))?,
            None => Vec::new(),
        };

        Ok(())
    }
}