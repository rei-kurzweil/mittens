use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Text shadow styling.
///
/// If a `TextShadowComponent` is parented to a `TextComponent`, `TextSystem` will spawn additional
/// shadow renderables for every glyph.
#[derive(Debug, Clone, Copy)]
pub struct TextShadowComponent {
    /// Shadow color (RGBA). Default: black.
    pub rgba: [f32; 4],

    /// Shadow scale multiplier. Default: 1.25.
    pub scale: f32,

    /// Shadow XYZ offset in glyph-local space.
    ///
    /// For Z: `TextSystem` uses this as a *magnitude* to nudge the shadow behind the main glyph
    /// to avoid z-fighting.
    /// Default: [0.0, 0.0, 0.001].
    pub offset: [f32; 3],

    component: Option<ComponentId>,
}

impl TextShadowComponent {
    pub const DEFAULT_RGBA: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    pub const DEFAULT_SCALE: f32 = 1.25;
    pub const DEFAULT_OFFSET: [f32; 3] = [0.0, 0.0, 0.001];

    pub fn new() -> Self {
        Self {
            rgba: Self::DEFAULT_RGBA,
            scale: Self::DEFAULT_SCALE,
            offset: Self::DEFAULT_OFFSET,
            component: None,
        }
    }

    pub fn with_rgba(mut self, rgba: [f32; 4]) -> Self {
        self.rgba = rgba;
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_offset(mut self, offset: [f32; 3]) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_offset_xy(mut self, offset: [f32; 2]) -> Self {
        self.offset[0] = offset[0];
        self.offset[1] = offset[1];
        self
    }

    pub fn with_z_offset(mut self, z_offset: f32) -> Self {
        self.offset[2] = z_offset;
        self
    }
}

impl Default for TextShadowComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TextShadowComponent {
    fn name(&self) -> &'static str {
        "text_shadow"
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

    fn init(&mut self, _emit: &mut dyn crate::engine::ecs::SignalEmitter, _component: ComponentId) {
        // TextShadow is consumed by TextSystem at TextComponent expansion time.
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("rgba".to_string(), serde_json::json!(self.rgba));
        map.insert("scale".to_string(), serde_json::json!(self.scale));
        map.insert("offset".to_string(), serde_json::json!(self.offset));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(rgba) = data.get("rgba") {
            self.rgba = serde_json::from_value(rgba.clone())
                .map_err(|e| format!("Failed to decode rgba: {}", e))?;
        } else {
            self.rgba = Self::DEFAULT_RGBA;
        }

        if let Some(scale) = data.get("scale") {
            self.scale = serde_json::from_value(scale.clone())
                .map_err(|e| format!("Failed to decode scale: {}", e))?;
        } else {
            self.scale = Self::DEFAULT_SCALE;
        }

        if let Some(offset) = data.get("offset") {
            self.offset = serde_json::from_value(offset.clone())
                .map_err(|e| format!("Failed to decode offset: {}", e))?;
        } else {
            self.offset = Self::DEFAULT_OFFSET;
        }

        Ok(())
    }
}
