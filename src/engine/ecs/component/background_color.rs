use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Global background/clear color.
///
/// This is intended to be a singleton-like component (the last registered wins).
#[derive(Debug, Clone, Copy)]
pub struct BackgroundColorComponent {
    pub rgba: [f32; 4],
}

impl BackgroundColorComponent {
    pub fn new() -> Self {
        Self {
            rgba: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { rgba: [r, g, b, a] }
    }

    pub fn with_rgba(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.rgba = [r, g, b, a];
        self
    }
}

impl Default for BackgroundColorComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for BackgroundColorComponent {
    fn name(&self) -> &'static str {
        "background_color"
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
            crate::engine::ecs::IntentValue::RegisterBackgroundColor {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("rgba".to_string(), serde_json::json!(self.rgba));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(rgba) = data.get("rgba") {
            self.rgba = serde_json::from_value(rgba.clone())
                .map_err(|e| format!("Failed to decode rgba: {}", e))?;
        }
        Ok(())
    }
}
