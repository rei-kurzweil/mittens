use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::MsaaMode;

/// Global renderer settings.
///
/// This is intended to be a singleton-like component (the last registered wins).
#[derive(Debug, Clone, Copy)]
pub struct RendererSettingsComponent {
    pub msaa4x: bool,
}

impl RendererSettingsComponent {
    pub fn new() -> Self {
        Self { msaa4x: true }
    }

    pub fn msaa_off() -> Self {
        Self { msaa4x: false }
    }

    pub fn msaa_mode(&self) -> MsaaMode {
        if self.msaa4x {
            MsaaMode::Msaa4x
        } else {
            MsaaMode::Off
        }
    }
}

impl Default for RendererSettingsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for RendererSettingsComponent {
    fn name(&self) -> &'static str {
        "renderer_settings"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterRendererSettings {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("msaa4x".to_string(), serde_json::json!(self.msaa4x));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(msaa4x) = data.get("msaa4x") {
            self.msaa4x = serde_json::from_value(msaa4x.clone())
                .map_err(|e| format!("Failed to decode msaa4x: {e}"))?;
        }
        Ok(())
    }
}
