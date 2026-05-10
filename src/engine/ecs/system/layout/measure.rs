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
#[derive(Clone)]
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
    /// Resolved `display` (style override → HtmlElement UA default → `None`).
    /// `None` is treated as `Block` by the block formatting context.
    pub display: Option<Display>,
}

/// Pass 1 — measure a single TC layout item.
///
/// Reads the [`StyleComponent`] from among `tc_id`'s ECS children and computes
/// the full box model (content, padding, margin) for both axes.
pub fn measure_item(world: &World, tc_id: ComponentId, avail_w_gu: f32) -> MeasuredItem {
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

    // Block (and inline-block) items with width: Auto fill the available width
    // after margins and padding. The inline cursor uses the flag to re-measure
    // an auto-width inline-block against the *remaining* line width at layout time.
    let is_auto_width = matches!(width, SizeDimension::Auto)
        && matches!(display, None | Some(Display::Block | Display::InlineBlock | Display::Inline));
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

    let is_inline_block = matches!(display, Some(Display::InlineBlock | Display::Inline));
    let content_height_gu = match height {
        SizeDimension::GlyphUnits(h) => h,
        SizeDimension::Auto if is_block || is_inline_block => {
            intrinsic_block_height(world, tc_id, content_width_gu)
        }
        SizeDimension::Auto => 0.0,
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
        content_width_gu,
        padding_left_gu,
        padding_right_gu,
        margin_left_gu,
        margin_right_gu,
        box_width_gu,
        margin_box_width_gu,
        is_auto_width,
        display,
    }
}

pub(crate) fn measure_container_items(
    world: &World,
    container_id: ComponentId,
    avail_w_gu: f32,
    _avail_h_gu: Option<f32>,
) -> Vec<MeasuredItem> {
    let children: Vec<ComponentId> = world.children_of(container_id).to_vec();
    children
        .into_iter()
        .filter(|&child| {
            world.get_component_by_id_as::<TransformComponent>(child).is_some()
                && !world
                    .component_label(child)
                    .map(|label| label.starts_with("__"))
                    .unwrap_or(false)
                && is_layout_item(world, child)
        })
        .map(|child| measure_item(world, child, avail_w_gu))
        .collect()
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

/// Glyph character advance in glyph units. Must match the renderer's
/// per-char x advance in `TextSystem::register_text` (1.0 col per glyph).
const CHAR_WIDTH_GU: f32 = 1.0;

/// Walk the subtree of `root` and return the `ComponentId` of the first
/// `TextComponent` found within local content (not crossing nested TCs).
pub(crate) fn find_text_id_in_local_content_subtree(
    world: &World,
    root: ComponentId,
) -> Option<ComponentId> {
    fn visit(world: &World, node: ComponentId, root: ComponentId) -> Option<ComponentId> {
        if world.get_component_by_id_as::<TextComponent>(node).is_some() {
            return Some(node);
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

/// If `tc_id` has a descendant `TextComponent` in its local content subtree,
/// narrow its `wrap_at` to fit `content_width_gu` and trigger a glyph rebuild.
///
/// Layout pass-only. Glyphs are built at TextComponent registration with the
/// authored default wrap_at (40), but layout knows the real container width;
/// without this, text overflows narrow boxes.
pub(crate) fn apply_text_wrap_for_item(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    tc_id: ComponentId,
    content_width_gu: f32,
) {
    let Some(text_id) = find_text_id_in_local_content_subtree(world, tc_id) else {
        return;
    };
    if content_width_gu <= CHAR_WIDTH_GU {
        return;
    }
    let container_cols = (content_width_gu / CHAR_WIDTH_GU).floor() as usize;
    let container_cols = container_cols.max(1);

    let (current_wrap_at, current_text) = match world.get_component_by_id_as::<TextComponent>(text_id) {
        Some(tc) => (tc.wrap_at, tc.text.clone()),
        None => return,
    };
    let new_wrap_at = container_cols.min(current_wrap_at);
    if new_wrap_at == current_wrap_at {
        return;
    }
    if let Some(tc) = world.get_component_by_id_as_mut::<TextComponent>(text_id) {
        tc.wrap_at = new_wrap_at;
    }
    emit.push_intent_now(
        text_id,
        crate::engine::ecs::IntentValue::SetText {
            component_ids: vec![text_id],
            text: current_text,
        },
    );
}

fn intrinsic_block_height(world: &World, tc_id: ComponentId, content_width_gu: f32) -> f32 {
    if find_text_in_local_content_subtree(world, tc_id).is_some() {
        return text_intrinsic_height(world, tc_id, content_width_gu);
    }

    let child_items = measure_container_items(world, tc_id, content_width_gu, None);
    if !child_items.is_empty() {
        let all_inline = child_items
            .iter()
            .all(|it| matches!(it.display, Some(Display::InlineBlock | Display::Inline)));
        if all_inline {
            // Inline-flow: simulate horizontal cursor + line wrap, sum line heights.
            let mut cursor_x = 0.0_f32;
            let mut line_h = 0.0_f32;
            let mut total_h = 0.0_f32;
            for item in &child_items {
                if cursor_x > 0.0 && cursor_x + item.margin_box_width_gu > content_width_gu {
                    total_h += line_h;
                    cursor_x = 0.0;
                    line_h = 0.0;
                }
                cursor_x += item.margin_box_width_gu;
                if item.margin_box_height_gu > line_h {
                    line_h = item.margin_box_height_gu;
                }
            }
            total_h += line_h;
            return total_h;
        }
        return child_items.iter().map(|item| item.margin_box_height_gu).sum();
    }

    descendant_layout_intrinsic_height(world, tc_id).unwrap_or(0.0)
}

fn descendant_layout_intrinsic_height(world: &World, root: ComponentId) -> Option<f32> {
    let mut total_height = 0.0;
    let mut found_height = false;

    for &child in world.children_of(root) {
        if world.get_component_by_id_as::<LayoutComponent>(child).is_some() {
            let (items, _, _, _) = measure_items(world, child);
            let layout_height: f32 = items.iter().map(|item| item.margin_box_height_gu).sum();
            if layout_height > 0.0 {
                total_height += layout_height;
                found_height = true;
                continue;
            }
        }

        if let Some(height) = descendant_layout_intrinsic_height(world, child) {
            total_height += height;
            found_height = true;
        }
    }

    found_height.then_some(total_height)
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
    use super::{measure_item, measure_items};
    use crate::engine::ecs::component::{ColorComponent, LayoutComponent, StyleComponent, TextComponent, TransformComponent};
    use crate::engine::ecs::component::style::SizeDimension;
    use crate::engine::ecs::World;

    #[test]
    fn auto_height_container_does_not_measure_text_behind_nested_transforms() {
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
        assert_eq!(measured.content_height_gu, 1.0);
    }

    #[test]
    fn block_auto_height_uses_intrinsic_child_item_height_instead_of_sharing_remaining_space() {
        let mut world = World::default();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(12.0));

        let container = world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let container_style = world.add_component_boxed_named("content_style", Box::new(StyleComponent::new()));

        let child = world.add_component_boxed_named("child", Box::new(TransformComponent::new()));
        let child_style = world.add_component_boxed_named(
            "child_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::GlyphUnits(3.0);
                s
            }),
        );

        let sibling = world.add_component_boxed_named("sibling", Box::new(TransformComponent::new()));
        let sibling_style = world.add_component_boxed_named(
            "sibling_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::GlyphUnits(2.0);
                s
            }),
        );

        let _ = world.add_child(root, container);
        let _ = world.add_child(container, container_style);
        let _ = world.add_child(container, child);
        let _ = world.add_child(child, child_style);
        let _ = world.add_child(root, sibling);
        let _ = world.add_child(sibling, sibling_style);

        let (items, _, _, _) = measure_items(&world, root);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content_height_gu, 3.0);
        assert_eq!(items[0].margin_box_height_gu, 3.0);
        assert_eq!(items[1].content_height_gu, 2.0);
    }

    #[test]
    fn block_auto_height_uses_descendant_layout_text_height_with_wrap() {
        let mut world = World::default();

        let container = world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let container_style = world.add_component_boxed_named("content_style", Box::new(StyleComponent::new()));
        let panel = world.add_component_boxed_named("world_panel", Box::new(ColorComponent::rgba(0.0, 0.0, 0.0, 0.0)));
        let scroll = world.add_component_boxed_named("world_panel_scroll", Box::new(ColorComponent::rgba(0.0, 0.0, 0.0, 0.0)));
        let rows_track = world.add_component_boxed_named("rows_track", Box::new(TransformComponent::new()));
        let rows_layout = world.add_component_boxed_named("rows_layout", Box::new(LayoutComponent::new(4.0)));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component_boxed_named("row_style", Box::new(StyleComponent::new()));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let text = world.add_component_boxed_named("row_text", Box::new(TextComponent::new("hello world hello world")));

        let _ = world.add_child(container, container_style);
        let _ = world.add_child(container, panel);
        let _ = world.add_child(panel, scroll);
        let _ = world.add_child(scroll, rows_track);
        let _ = world.add_child(rows_track, rows_layout);
        let _ = world.add_child(rows_layout, row);
        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, color);
        let _ = world.add_child(color, text);

        let measured = measure_item(&world, container, 4.0);
        assert!(measured.content_height_gu > 1.0, "wrapped descendant layout text should increase intrinsic height");
    }
}
