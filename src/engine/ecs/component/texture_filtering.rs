use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::TextureFiltering;

#[derive(Debug, Clone, Copy)]
pub struct TextureFilteringComponent {
    pub filtering: TextureFiltering,
}

impl TextureFilteringComponent {
    pub fn new(filtering: TextureFiltering) -> Self {
        Self { filtering }
    }

    pub fn linear() -> Self {
        Self::new(TextureFiltering::Linear)
    }

    pub fn nearest() -> Self {
        Self::new(TextureFiltering::Nearest)
    }

    pub fn nearest_magnification() -> Self {
        Self::new(TextureFiltering::NearestMagnification)
    }
}

impl Default for TextureFilteringComponent {
    fn default() -> Self {
        Self::linear()
    }
}

impl Component for TextureFilteringComponent {
    fn name(&self) -> &'static str {
        "texture_filtering"
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
            crate::engine::ecs::IntentValue::RegisterTextureFiltering {
                component_ids: vec![component],
            },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let mode = match self.filtering {
            TextureFiltering::Linear => "linear",
            TextureFiltering::Nearest => "nearest",
            TextureFiltering::NearestMagnification => "nearest_magnification",
        };
        map.insert("mode".to_string(), serde_json::json!(mode));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(mode) = data.get("mode") {
            let mode_str: String = serde_json::from_value(mode.clone())
                .map_err(|e| format!("Failed to decode mode: {}", e))?;
            self.filtering = match mode_str.as_str() {
                "linear" => TextureFiltering::Linear,
                "nearest" => TextureFiltering::Nearest,
                "nearest_magnification" | "nearestMag" | "nearest_mag" => {
                    TextureFiltering::NearestMagnification
                }
                other => {
                    return Err(format!(
                        "Invalid texture filtering mode '{other}'. Expected: linear|nearest|nearest_magnification"
                    ));
                }
            };
        }
        Ok(())
    }
}
