use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Global background/clear color.
///
/// This is a singleton-like marker component (the last registered wins).
/// Color is supplied by attaching a `ColorComponent` as a direct child.
/// If no `ColorComponent` child is present, the clear color defaults to opaque black.
///
/// ```text
/// BackgroundColorComponent
///   ColorComponent   ← sets the clear color
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct BackgroundColorComponent;

impl BackgroundColorComponent {
    pub fn new() -> Self {
        Self
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
        std::collections::HashMap::new()
    }

    fn decode(
        &mut self,
        _data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        Ok(())
    }
}
