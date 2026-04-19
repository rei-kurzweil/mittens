use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{HtmlElementComponent, LayoutComponent, StyleComponent, TextComponent, TransformComponent};
use crate::engine::ecs::component::style::{Display, SizeDimension};
use crate::engine::ecs::system::text_system::TextSystem;

/// Measured size of a single layout item after Pass 1.
///
/// Computed by [`measure_item`] from the item's [`StyleComponent`]; consumed
/// by the display-mode layout functions (`block`, `flex`, `inline`) in Pass 2.
///
/// All values are in **glyph units**.
pub(crate) struct MeasuredItem {
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
    /// true → height: Auto; gets a share of remaining container space in Pass 1
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
    /// true → width: Auto; item stretches to fill container inline axis
    pub is_auto_width:        bool,
}

/// Pass 1 — measure a single TC layout item.
///
/// Reads the [`StyleComponent`] from among `tc_id`'s ECS children and computes
/// the full box model (content, padding, margin) for both axes.
///
/// Auto heights are left with `content_height_gu = 0` and `is_auto_height = true`;
/// callers must resolve them after summing fixed items (see [`measure_items`]).
pub(crate) fn measure_item(world: &World, tc_id: ComponentId, avail_w_gu: f32) -> MeasuredItem {
    // Copy style fields out before the borrow ends.
    let children: Vec<ComponentId> = world.children_of(tc_id).to_vec();
    let style = children.iter().find_map(|&child| {
        world.get_component_by_id_as::<StyleComponent>(child).map(|s| {
            (s.padding, s.margin, s.height, s.width, s.display, s.flex_grow)
        })
    });

    let (padding, margin, height, width, style_display, _flex_grow) = style.unwrap_or_default();

    // Resolve display: StyleComponent.display overrides HtmlElementComponent UA default.
    let ua_display = children.iter().find_map(|&child| {
        world.get_component_by_id_as::<HtmlElementComponent>(child)
            .and_then(|el| el.element_type.default_display())
    });
    let display = style_display.or(ua_display);

    let is_block = matches!(display, None | Some(Display::Block));

    // ── Horizontal ───────────────────────────────────────────────────────
    let margin_left_gu   = margin.left;
    let margin_right_gu  = margin.right;
    let padding_left_gu  = padding.left;
    let padding_right_gu = padding.right;

    // Block elements with width: Auto fill the available width after margins and padding.
    let is_auto_width = is_block && matches!(width, SizeDimension::Auto);
    let content_width_gu = match width {
        SizeDimension::GlyphUnits(w) => w,
        _ => (avail_w_gu - margin_left_gu - margin_right_gu
                         - padding_left_gu - padding_right_gu).max(0.0),
    };
    let box_width_gu        = padding_left_gu + content_width_gu + padding_right_gu;
    let margin_box_width_gu = margin_left_gu + box_width_gu + margin_right_gu;

    // ── Vertical ─────────────────────────────────────────────────────────
    let margin_top_gu    = margin.top;
    let margin_bottom_gu = margin.bottom;
    let padding_top_gu   = padding.top;
    let padding_bottom_gu = padding.bottom;

    // is_auto_height = true means "take a share of remaining container space".
    // Text nodes with height: Auto resolve intrinsically (not from container),
    // so they are NOT auto from the container's perspective.
    let has_text = find_text_in_local_content_subtree(world, tc_id).is_some();
    let is_auto_height = is_block && matches!(height, SizeDimension::Auto) && !has_text;
    let content_height_gu = match height {
        SizeDimension::GlyphUnits(h) => h,
        SizeDimension::Auto => {
            // Intrinsic height: if the subtree contains a TextComponent, measure it.
            // wrap_at is derived from the available content width in character columns.
            text_intrinsic_height(world, tc_id, content_width_gu)
        }
        _ => 0.0,
    };
    let box_height_gu        = padding_top_gu + content_height_gu + padding_bottom_gu;
    let margin_box_height_gu = margin_top_gu + box_height_gu + margin_bottom_gu;

    MeasuredItem {
        tc_id,
        content_height_gu,
        padding_top_gu,
        padding_bottom_gu,
        margin_top_gu,
        margin_bottom_gu,
        box_height_gu,
        margin_box_height_gu,
        is_auto_height,
        content_width_gu,
        padding_left_gu,
        padding_right_gu,
        margin_left_gu,
        margin_right_gu,
        box_width_gu,
        margin_box_width_gu,
        is_auto_width,
    }
}

pub(crate) fn measure_container_items(
    world: &World,
    container_id: ComponentId,
    avail_w_gu: f32,
    avail_h_gu: Option<f32>,
) -> Vec<MeasuredItem> {
    let children: Vec<ComponentId> = world.children_of(container_id).to_vec();
    let mut items: Vec<MeasuredItem> = children
        .into_iter()
        .filter(|&child| {
            world.get_component_by_id_as::<TransformComponent>(child).is_some()
                && world.component_label(child) != Some("__bg")
                && is_layout_item(world, child)
        })
        .map(|child| measure_item(world, child, avail_w_gu))
        .collect();

    if let Some(h) = avail_h_gu {
        let total_fixed: f32 = items
            .iter()
            .filter(|i| !i.is_auto_height)
            .map(|i| i.margin_box_height_gu)
            .sum();
        let count_auto = items.iter().filter(|i| i.is_auto_height).count();

        if count_auto > 0 {
            let remaining = (h - total_fixed).max(0.0);
            let auto_margin_box = remaining / count_auto as f32;

            for item in items.iter_mut().filter(|i| i.is_auto_height) {
                item.margin_box_height_gu = auto_margin_box;
                item.box_height_gu = (auto_margin_box
                    - item.margin_top_gu
                    - item.margin_bottom_gu).max(0.0);
                item.content_height_gu = (item.box_height_gu
                    - item.padding_top_gu
                    - item.padding_bottom_gu).max(0.0);
            }
        }
    }

    items
}

fn is_layout_item(world: &World, tc_id: ComponentId) -> bool {
    world.children_of(tc_id).iter().any(|&child| {
        world.get_component_by_id_as::<StyleComponent>(child).is_some()
            || world.get_component_by_id_as::<HtmlElementComponent>(child).is_some()
    })
}

/// Pass 1 — measure all TC children of a [`LayoutComponent`] root.
///
/// Returns `(items, avail_w_gu, avail_h_gu, unit_scale)`.
///
/// Auto-height items are resolved against the container's `available_height`
/// (if set) before returning — callers receive fully resolved `MeasuredItem`s
/// and do not need to re-run the distribution logic.
///
/// If `available_height` is `None`, auto-height items retain `content_height_gu = 0`
/// (intrinsic / content-driven sizing is not yet implemented).
pub(crate) fn measure_items(
    world: &World,
    layout_id: ComponentId,
) -> (Vec<MeasuredItem>, f32, Option<f32>, f32) {
    let (avail_w, avail_h, unit_scale) = {
        let lc = match world.get_component_by_id_as::<LayoutComponent>(layout_id) {
            Some(l) => l,
            None => return (Vec::new(), 0.0, None, 1.0),
        };
        (lc.available_width, lc.available_height, lc.unit_scale)
    };

    let items = measure_container_items(world, layout_id, avail_w, avail_h);

    (items, avail_w, avail_h, unit_scale)
}

// ── Text intrinsic height ─────────────────────────────────────────────────────

/// Approximate glyph character width in glyph units (matches TextSystem rendering).
const CHAR_WIDTH_GU: f32 = 0.55;

/// Walk the subtree of `root` and return the first `TextComponent` found.
fn find_text_in_local_content_subtree(
    world: &World,
    root: ComponentId,
) -> Option<(String, usize, bool, Vec<String>)> {
    fn visit(
        world: &World,
        node: ComponentId,
        root: ComponentId,
    ) -> Option<(String, usize, bool, Vec<String>)> {
        if let Some(t) = world.get_component_by_id_as::<TextComponent>(node) {
            return Some((
                t.text.clone(),
                t.wrap_at,
                t.word_wrap,
                t.word_wrap_tokens.clone(),
            ));
        }

        if node != root && world.get_component_by_id_as::<TransformComponent>(node).is_some() {
            return None;
        }

        for &child in world.children_of(node) {
            if let Some(found) = visit(world, child, root) {
                return Some(found);
            }
        }

        None
    }

    visit(world, root, root)
}

/// Measure the intrinsic block-axis height (in glyph units) of a TC subtree
/// by finding its `TextComponent` and running `TextSystem::measure`.
///
/// Returns `0.0` if no `TextComponent` is found in the subtree.
fn text_intrinsic_height(world: &World, tc_id: ComponentId, content_width_gu: f32) -> f32 {
    let Some((text, existing_wrap_at, word_wrap, tokens)) =
        find_text_in_local_content_subtree(world, tc_id)
    else {
        return 0.0;
    };

    // Derive wrap_at from available width if the content area is known and wider
    // than a single character; otherwise fall back to the TextComponent's own wrap_at.
    // Use the container-derived wrap_at, but never exceed the TextComponent's own
    // wrap_at — the TextSystem will use that limit, so measuring with a larger value
    // would undercount lines for texts that hit the TC's hard-wrap point.
    let wrap_at = if content_width_gu > CHAR_WIDTH_GU {
        let container_cols = (content_width_gu / CHAR_WIDTH_GU).floor() as usize;
        container_cols.min(existing_wrap_at)
    } else {
        existing_wrap_at
    };

    let (_max_col, line_count) = TextSystem::measure(&text, wrap_at.max(1), word_wrap, &tokens);
    line_count as f32 // 1.0 gu per line; caller multiplies by style.line_height if needed
}

// ── Default style values used when no StyleComponent is present ───────────────

use crate::engine::ecs::component::style::EdgeInsets;

type StyleTuple = (EdgeInsets, EdgeInsets, SizeDimension, SizeDimension, Option<Display>, f32);

trait StyleDefault {
    fn unwrap_or_default(self) -> StyleTuple;
}

impl StyleDefault for Option<StyleTuple> {
    fn unwrap_or_default(self) -> StyleTuple {
        self.unwrap_or((
            EdgeInsets::ZERO,
            EdgeInsets::ZERO,
            SizeDimension::Auto,
            SizeDimension::Auto,
            None,
            0.0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::measure_item;
    use crate::engine::ecs::component::{ColorComponent, LayoutComponent, StyleComponent, TextComponent, TransformComponent};
    use crate::engine::ecs::component::style::SizeDimension;
    use crate::engine::ecs::World;

    #[test]
    fn auto_height_container_ignores_text_behind_nested_transforms() {
        let mut world = World::default();

        let container = world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named("content_style", Box::new(StyleComponent::new()));
        let panel = world.add_component_boxed_named("world_panel", Box::new(LayoutComponent::new(10.0)));
        let rows_track = world.add_component_boxed_named("rows_track", Box::new(TransformComponent::new()));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let text = world.add_component_boxed_named("row_text", Box::new(TextComponent::new("hello")));

        let _ = world.add_child(container, style);
        let _ = world.add_child(container, panel);
        let _ = world.add_child(panel, rows_track);
        let _ = world.add_child(rows_track, row);
        let _ = world.add_child(row, color);
        let _ = world.add_child(color, text);

        let measured = measure_item(&world, container, 29.5);
        assert!(measured.is_auto_height, "container should remain auto-height/flex-sized");
        assert_eq!(measured.content_height_gu, 0.0);
    }

    #[test]
    fn row_text_wrapper_still_measures_intrinsic_height() {
        let mut world = World::default();

        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component_boxed_named(
            "row_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::Auto;
                s
            }),
        );
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let text = world.add_component_boxed_named("row_text", Box::new(TextComponent::new("hello")));

        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, color);
        let _ = world.add_child(color, text);

        let measured = measure_item(&world, row, 12.0);
        assert!(!measured.is_auto_height, "row text wrapper should use intrinsic text height");
        assert_eq!(measured.content_height_gu, 1.0);
    }
}
