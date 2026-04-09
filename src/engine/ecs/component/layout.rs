use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// The viewport of a self-contained layout subtree — analogous to the browser's
/// initial containing block.
///
/// `LayoutComponent` does **not** participate in flow itself; it defines the space
/// available to the first `HtmlElementComponent` child (usually `Body`).
///
/// Multiple `LayoutComponent` nodes can coexist — one per panel, one per HUD region,
/// one per workspace.
///
/// `available_width` is in **glyph units** (1.0 = one monospace character cell).
/// World-space scaling stays in `TransformComponent`.
///
/// Set `dirty = true` to signal the `LayoutSystem` to recompute the subtree on the next tick.
#[derive(Debug, Clone)]
pub struct LayoutComponent {
    /// Available inline (X-axis) width for children, in glyph units.
    pub available_width: f32,

    /// Optional block (Y-axis) constraint. Used for overflow/clip; `None` = unconstrained.
    pub available_height: Option<f32>,

    /// When `true`, the layout system will recompute this subtree on the next tick.
    pub dirty: bool,

    /// Scale factor to convert glyph units → local coordinates of the nearest ancestor
    /// `TransformComponent`.
    ///
    /// **When the parent `TransformComponent` already has `scale = TEXT_SCALE`** (i.e. the whole
    /// subtree is in glyph-unit space), leave this at the default `1.0`.
    ///
    /// **When the parent `TransformComponent` has `scale = 1.0` (world units)** and the
    /// `StyleComponent` heights are authored in glyph units, set `unit_scale = TEXT_SCALE`
    /// (e.g. `0.08`) so the emitted `UpdateTransform` translations land in world space.
    pub unit_scale: f32,

    component: Option<ComponentId>,
}

impl LayoutComponent {
    pub fn new(available_width: f32) -> Self {
        Self {
            available_width,
            available_height: None,
            dirty: true,
            unit_scale: 1.0,
            component: None,
        }
    }

    pub fn with_height(mut self, h: f32) -> Self {
        self.available_height = Some(h);
        self
    }

    pub fn with_unit_scale(mut self, scale: f32) -> Self {
        self.unit_scale = scale;
        self
    }

    /// Mark this layout root as needing a recompute.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

impl Component for LayoutComponent {
    fn name(&self) -> &'static str { "layout" }

    fn set_id(&mut self, id: ComponentId) { self.component = Some(id); }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("available_width".to_string(), serde_json::json!(self.available_width));
        if let Some(h) = self.available_height {
            map.insert("available_height".to_string(), serde_json::json!(h));
        }
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("available_width").and_then(|v| v.as_f64()) {
            self.available_width = v as f32;
            self.dirty = true;
        }
        if let Some(v) = data.get("available_height").and_then(|v| v.as_f64()) {
            self.available_height = Some(v as f32);
        }
        Ok(())
    }
}
