use crate::engine::ecs::component::Component;

/// Controls which pointer event types a raycastable captures vs. passes through.
///
/// When the gesture system walks the depth-sorted hit list, it stops at the first hit that
/// captures the event type being resolved. Objects behind a capturer never see that event type.
///
/// ```
/// depth-sorted hits: [drag_plane (DragOnly), row (All)]
///
/// for drag  → drag_plane captures, stops. row never sees DragStart/DragMove.
/// for click → drag_plane passes (DragOnly). row captures, stops.
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEvents {
    /// Captures drag and click. Default for all raycastable geometry.
    #[default]
    All,
    /// Captures drag events only; click propagates to the next hit.
    DragOnly,
    /// Captures click events only; drag propagates to the next hit.
    ClickOnly,
    /// Passes all pointer events through. Geometry is hittable but invisible to the gesture
    /// system (e.g. a structural collision volume that should never receive input).
    PassThrough,
}

impl PointerEvents {
    pub fn captures_drag(self) -> bool {
        matches!(self, Self::All | Self::DragOnly)
    }
    pub fn captures_click(self) -> bool {
        matches!(self, Self::All | Self::ClickOnly)
    }
}

/// Controls whether renderables should be eligible for ray casting (BVH insertion).
///
/// This is intentionally separate from `RenderableComponent` so raycasting policy can be
/// expressed via topology/components rather than renderable data.
#[derive(Debug, Default, Clone, Copy)]
pub struct RaycastableComponent {
    /// If true, ray casting is enabled.
    pub enable: bool,
    /// Which pointer event types this object captures vs. passes through to hits behind it.
    pub pointer_events: PointerEvents,
}

impl RaycastableComponent {
    pub fn new(enable: bool) -> Self {
        Self { enable, pointer_events: PointerEvents::All }
    }

    pub fn enabled() -> Self {
        Self::new(true)
    }

    pub fn disabled() -> Self {
        Self::new(false)
    }

    /// Captures drag events only; click falls through to hits behind this object.
    pub fn drag_only() -> Self {
        Self { enable: true, pointer_events: PointerEvents::DragOnly }
    }

    /// Captures click events only; drag falls through to hits behind this object.
    pub fn click_only() -> Self {
        Self { enable: true, pointer_events: PointerEvents::ClickOnly }
    }
}

impl Component for RaycastableComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "raycastable"
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("enable".to_string(), serde_json::json!(self.enable));
        let pe = match self.pointer_events {
            PointerEvents::All => "all",
            PointerEvents::DragOnly => "drag_only",
            PointerEvents::ClickOnly => "click_only",
            PointerEvents::PassThrough => "pass_through",
        };
        map.insert("pointer_events".to_string(), serde_json::json!(pe));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("enable") {
            if let Some(b) = v.as_bool() {
                self.enable = b;
            }
        }
        if let Some(v) = data.get("pointer_events").and_then(|v| v.as_str()) {
            self.pointer_events = match v {
                "drag_only" => PointerEvents::DragOnly,
                "click_only" => PointerEvents::ClickOnly,
                "pass_through" => PointerEvents::PassThrough,
                _ => PointerEvents::All,
            };
        }
        Ok(())
    }
}
