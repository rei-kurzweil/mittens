use crate::engine::ecs::ComponentId;

/// Non-ECS scroll state for a layout-native scroll container.
///
/// Owned by `ScrollSystem` in a `HashMap<ComponentId, SharedScrollState>` keyed by
/// container_tc (the TC with `StyleComponent { overflow: Scroll }`).
/// Not stored in the ECS world — internal to the scroll/layout systems only.
#[derive(Debug, Default)]
pub struct ScrollState {
    /// Accumulated scroll in world units. Always ≤ 0 (0 = top).
    pub scroll_y: f32,

    /// Total content height in world units — updated by LayoutSystem after each layout pass.
    pub content_height: f32,

    /// Visible viewport height in world units — set when the scroll region is registered.
    pub viewport_height: f32,

    /// ComponentId of the inner scroll track TC (parent of all scrollable children).
    pub scroll_track: Option<ComponentId>,
}

impl ScrollState {
    /// Maximum downward scroll (always positive; scroll_y clamps to −max_scroll..=0).
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }
}
