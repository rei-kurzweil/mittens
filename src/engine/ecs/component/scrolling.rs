use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Virtual scroll state for a list panel.
///
/// Holds configuration and runtime scroll position only — item data lives in the owning system.
///
/// Topology (inside a panel):
/// ```text
/// panel_component
///   └── ScrollingComponent        ← DragMove + ScrollChanged handlers registered here
///         └── rows_anchor         ← only page_size row children live here
///               ├── row_0
///               └── ...
/// ```
///
/// The owning system registers a `DragMove` handler that calls `apply_drag` and a
/// `ScrollChanged` handler that rebuilds the visible row window.
#[derive(Debug, Clone)]
pub struct ScrollingComponent {
    /// Height of each item in overlay world units.
    pub item_height: f32,
    /// Maximum number of items rendered at once (the visible page).
    pub page_size: usize,
    /// Total logical item count (kept in sync by the owning system).
    pub total_items: usize,

    /// Continuous scroll position in items. 0.0 = top.
    pub scroll_offset: f32,
    /// Window start the last time `ScrollChanged` was emitted — used for boundary detection.
    pub(crate) last_window_start: usize,

    component: Option<ComponentId>,
}

impl ScrollingComponent {
    pub fn new(item_height: f32, page_size: usize) -> Self {
        Self {
            item_height,
            page_size,
            total_items: 0,
            scroll_offset: 0.0,
            last_window_start: 0,
            component: None,
        }
    }

    /// First visible item index (inclusive).
    pub fn window_start(&self) -> usize {
        self.scroll_offset.floor() as usize
    }

    /// Last visible item index (exclusive).
    pub fn window_end(&self) -> usize {
        (self.window_start() + self.page_size).min(self.total_items)
    }

    /// Maximum scroll value in items.
    pub fn max_scroll(&self) -> f32 {
        (self.total_items.saturating_sub(self.page_size)) as f32
    }

    /// Update `scroll_offset` by a world-space Y drag delta and return `Some((start, end))` if
    /// the window boundary was crossed (caller should emit `ScrollChanged`).
    ///
    /// Sign convention: dragging up (positive `delta_y`) reveals items lower in the list.
    pub fn apply_drag(&mut self, delta_y: f32) -> Option<(usize, usize)> {
        if self.item_height <= 0.0 {
            return None;
        }
        let prev_start = self.window_start();
        self.scroll_offset -= delta_y / self.item_height;
        self.scroll_offset = self.scroll_offset.clamp(0.0, self.max_scroll());
        let new_start = self.window_start();
        if new_start != prev_start {
            self.last_window_start = new_start;
            Some((new_start, self.window_end()))
        } else {
            None
        }
    }

    /// Clamp scroll after `total_items` changes. Returns true if the position changed.
    pub fn clamp_to_total(&mut self) -> bool {
        let clamped = self.scroll_offset.clamp(0.0, self.max_scroll());
        if (clamped - self.scroll_offset).abs() > f32::EPSILON {
            self.scroll_offset = clamped;
            true
        } else {
            false
        }
    }
}

impl Component for ScrollingComponent {
    fn name(&self) -> &'static str {
        "scrolling"
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
        map.insert("item_height".to_string(), serde_json::json!(self.item_height));
        map.insert("page_size".to_string(), serde_json::json!(self.page_size));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("item_height").and_then(|v| v.as_f64()) {
            self.item_height = v as f32;
        }
        if let Some(v) = data.get("page_size").and_then(|v| v.as_u64()) {
            self.page_size = v as usize;
        }
        Ok(())
    }
}
