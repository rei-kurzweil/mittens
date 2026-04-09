use crate::engine::ecs::ComponentId;

/// Measured size of a single layout item after Pass 1.
///
/// Computed by `measure_item` from the item's `StyleComponent`; consumed
/// by the display-mode layout functions (block, flex, inline) in Pass 2.
///
/// All values are in **glyph units**.
pub struct MeasuredItem {
    pub tc_id: ComponentId,

    // ── Vertical ─────────────────────────────────────────────────────────
    pub content_height_gu:    f32,
    pub padding_top_gu:       f32,
    pub padding_bottom_gu:    f32,
    pub margin_top_gu:        f32,
    pub margin_bottom_gu:     f32,
    /// padding_top + content_height + padding_bottom
    pub box_height_gu:        f32,
    /// margin_top + box_height + margin_bottom
    pub margin_box_height_gu: f32,
    /// true → height: Auto; gets a share of remaining container space
    pub is_auto_height:       bool,

    // ── Horizontal ───────────────────────────────────────────────────────
    pub content_width_gu:     f32,
    pub padding_left_gu:      f32,
    pub padding_right_gu:     f32,
    pub margin_left_gu:       f32,
    pub margin_right_gu:      f32,
    /// padding_left + content_width + padding_right
    pub box_width_gu:         f32,
    /// margin_left + box_width + margin_right
    pub margin_box_width_gu:  f32,
    /// true → width: Auto; stretches to fill container (block default)
    pub is_auto_width:        bool,
}

// Pass 1 implementation — TODO:
// `measure_item(world, tc_id, avail_w_gu) -> MeasuredItem`
// `measure_items(world, layout_id) -> (Vec<MeasuredItem>, avail_w, avail_h)`
// See docs/draft/layout-system-impl-plan.md Phase A.
