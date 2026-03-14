use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::MsaaMode;

/// Global renderer settings.
///
/// This is intended to be a singleton-like component (the last registered wins).
#[derive(Debug, Clone, Copy)]
pub struct RendererSettingsComponent {
    pub msaa4x: bool,
    pub window_size: Option<[u32; 2]>,
}

impl RendererSettingsComponent {
    pub fn new() -> Self {
        Self {
            msaa4x: true,
            window_size: None,
        }
    }

    pub fn msaa_off() -> Self {
        Self {
            msaa4x: false,
            window_size: None,
        }
    }

    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        if width > 0 && height > 0 {
            self.window_size = Some([width, height]);
        }
        self
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
        if let Some(window_size) = self.window_size {
            map.insert("window_size".to_string(), serde_json::json!(window_size));
        }
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
        if let Some(window_size) = data.get("window_size") {
            self.window_size = Some(
                serde_json::from_value(window_size.clone())
                    .map_err(|e| format!("Failed to decode window_size: {e}"))?,
            );
        }
        Ok(())
    }
}
