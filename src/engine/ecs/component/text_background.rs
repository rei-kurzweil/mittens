use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Attaches to a `TextComponent` to request a background quad behind the rendered text.
///
/// `TextSystem::register_text` detects this as an immediate child of a `TextComponent`
/// and spawns a centered, scaled `RenderableComponent::square()` behind the glyphs.
///
/// Padding is specified per-side in glyph-space units, mirroring CSS conventions.
/// Use `with_padding(v)` to set all four sides at once.
///
/// Topology spawned at build time (as children of the `TextComponent`):
/// ```text
/// TextComponent
///   TextBackgroundComponent   ← this node (marker/config)
///   TransformComponent        ← sized/positioned background
///     ColorComponent
///       RenderableComponent   ← the actual quad
///         OpacityComponent    ← routes into the transparent pass
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TextBackgroundComponent {
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,

    /// Background RGBA color. Alpha drives opacity.
    pub color: [f32; 4],

    /// Z offset relative to the text origin (negative = behind glyphs).
    pub z_offset: f32,

    component: Option<ComponentId>,
}

impl TextBackgroundComponent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set all four padding sides to the same value.
    pub fn with_padding(mut self, v: f32) -> Self {
        self.padding_top = v;
        self.padding_right = v;
        self.padding_bottom = v;
        self.padding_left = v;
        self
    }

    pub fn with_padding_top(mut self, v: f32) -> Self {
        self.padding_top = v;
        self
    }

    pub fn with_padding_right(mut self, v: f32) -> Self {
        self.padding_right = v;
        self
    }

    pub fn with_padding_bottom(mut self, v: f32) -> Self {
        self.padding_bottom = v;
        self
    }

    pub fn with_padding_left(mut self, v: f32) -> Self {
        self.padding_left = v;
        self
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }

    pub fn with_z_offset(mut self, z: f32) -> Self {
        self.z_offset = z;
        self
    }
}

impl Default for TextBackgroundComponent {
    fn default() -> Self {
        Self {
            padding_top: 0.35,
            padding_right: 0.5,
            padding_bottom: 0.35,
            padding_left: 0.5,
            color: [0.0, 0.0, 0.0, 0.75],
            z_offset: -0.1,
            component: None,
        }
    }
}

impl Component for TextBackgroundComponent {
    fn name(&self) -> &'static str {
        "text_background"
    }

    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("padding_top".to_string(), serde_json::json!(self.padding_top));
        map.insert(
            "padding_right".to_string(),
            serde_json::json!(self.padding_right),
        );
        map.insert(
            "padding_bottom".to_string(),
            serde_json::json!(self.padding_bottom),
        );
        map.insert(
            "padding_left".to_string(),
            serde_json::json!(self.padding_left),
        );
        map.insert("color".to_string(), serde_json::json!(self.color));
        map.insert("z_offset".to_string(), serde_json::json!(self.z_offset));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        // `padding` shorthand: sets all four sides.
        if let Some(v) = data.get("padding") {
            if let Some(f) = v.as_f64() {
                let p = f as f32;
                self.padding_top = p;
                self.padding_right = p;
                self.padding_bottom = p;
                self.padding_left = p;
            }
        }
        if let Some(v) = data.get("padding_top") {
            if let Some(f) = v.as_f64() {
                self.padding_top = f as f32;
            }
        }
        if let Some(v) = data.get("padding_right") {
            if let Some(f) = v.as_f64() {
                self.padding_right = f as f32;
            }
        }
        if let Some(v) = data.get("padding_bottom") {
            if let Some(f) = v.as_f64() {
                self.padding_bottom = f as f32;
            }
        }
        if let Some(v) = data.get("padding_left") {
            if let Some(f) = v.as_f64() {
                self.padding_left = f as f32;
            }
        }
        if let Some(v) = data.get("color") {
            self.color = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode color: {e}"))?;
        }
        if let Some(v) = data.get("z_offset") {
            if let Some(f) = v.as_f64() {
                self.z_offset = f as f32;
            }
        }
        Ok(())
    }
}
