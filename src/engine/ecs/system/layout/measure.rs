use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::style::{Display, SizeDimension, WordWrapMode};
use crate::engine::ecs::component::{
    BoundsComponent, ColorComponent, HtmlElementComponent, LayoutComponent, RenderableComponent,
    StyleComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::system::text_system::TextSystem;
use crate::engine::graphics::bounds::{Aabb, mat4_identity, mat4_mul};
use crate::engine::graphics::primitives::TransformMatrix;

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
    pub content_height_gu: f32,
    pub padding_top_gu: f32,
    pub padding_bottom_gu: f32,
    pub margin_top_gu: f32,
    pub margin_bottom_gu: f32,
    /// padding_top + content_height + padding_bottom
    pub box_height_gu: f32,
    /// margin_top + box_height + margin_bottom
    pub margin_box_height_gu: f32,

    // ── Horizontal ───────────────────────────────────────────────────────
    pub content_width_gu: f32,
    pub padding_left_gu: f32,
    pub padding_right_gu: f32,
    pub margin_left_gu: f32,
    pub margin_right_gu: f32,
    /// padding_left + content_width + padding_right
    pub box_width_gu: f32,
    /// margin_left + box_width + margin_right
    pub margin_box_width_gu: f32,
    /// true → width: Auto; item stretches to fill container inline axis
    pub is_auto_width: bool,
    /// Resolved `display` (style override → HtmlElement UA default → `None`).
    /// `None` is treated as `Block` by the block formatting context.
    pub display: Option<Display>,
}

fn len3(column: [f32; 4]) -> f32 {
    (column[0] * column[0] + column[1] * column[1] + column[2] * column[2]).sqrt()
}

fn layout_root_world_axis_scales(world: &World, layout_id: ComponentId) -> (f32, f32) {
    let mut chain: Vec<TransformMatrix> = Vec::new();
    let mut cur = Some(layout_id);

    while let Some(node) = cur {
        if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(node) {
            chain.push(tc.transform.model);
        }
        cur = world.parent_of(node);
    }

    chain.reverse();

    let mut world_model = mat4_identity();
    for model in chain {
        world_model = mat4_mul(world_model, model);
    }

    (
        len3(world_model[0]).max(1e-6),
        len3(world_model[1]).max(1e-6),
    )
}

fn resolve_layout_root_length_gu(
    length: SizeDimension,
    unit_scale: f32,
    axis_world_scale: f32,
) -> f32 {
    match length {
        SizeDimension::GlyphUnits(v) => v,
        SizeDimension::WorldUnits(v) => {
            let denom = unit_scale.abs().max(f32::EPSILON) * axis_world_scale.max(1e-6);
            v / denom
        }
        SizeDimension::Auto | SizeDimension::Percent(_) => {
            debug_assert!(false, "LayoutRoot sizes only support gu or wu units");
            0.0
        }
    }
}

pub(crate) fn layout_root_available_bounds(
    world: &World,
    layout_id: ComponentId,
) -> (f32, Option<f32>, f32) {
    let Some(lc) = world.get_component_by_id_as::<LayoutComponent>(layout_id) else {
        return (0.0, None, 1.0);
    };

    let (world_scale_x, world_scale_y) = layout_root_world_axis_scales(world, layout_id);
    let avail_w =
        resolve_layout_root_length_gu(lc.authored_available_width, lc.unit_scale, world_scale_x);
    let avail_h = lc
        .authored_available_height
        .map(|height| resolve_layout_root_length_gu(height, lc.unit_scale, world_scale_y));

    if trace_layout_id(world, layout_id) {
        println!(
            "[layout-trace] root={} id={:?} authored_w={:?} authored_h={:?} unit_scale={} ancestor_scale=({:.6},{:.6}) avail_gu=({:.6},{}) local_wu=({:.6},{}) final_wu=({:.6},{})",
            trace_label(world, layout_id),
            layout_id,
            lc.authored_available_width,
            lc.authored_available_height,
            lc.unit_scale,
            world_scale_x,
            world_scale_y,
            avail_w,
            fmt_opt(avail_h),
            avail_w * lc.unit_scale,
            fmt_opt(avail_h.map(|h| h * lc.unit_scale)),
            avail_w * lc.unit_scale * world_scale_x,
            fmt_opt(avail_h.map(|h| h * lc.unit_scale * world_scale_y)),
        );
    }

    (avail_w, avail_h, lc.unit_scale)
}

pub(crate) fn layout_root_axis_scales(world: &World, layout_id: ComponentId) -> (f32, f32) {
    layout_root_world_axis_scales(world, layout_id)
}

pub(crate) fn trace_layout_id(world: &World, id: ComponentId) -> bool {
    if let Some(label) = world.component_label(id) {
        if label == "text_input_demo"
            || label.starts_with("row_")
            || label == "content_slot"
            || label == "assets_content_area"
            || label == "asset_item"
            || label == "asset_item_root"
            || label.starts_with("item_")
        {
            return true;
        }
    }
    // Deep trace children of traced containers
    if let Some(parent) = world.parent_of(id) {
        return trace_layout_id(world, parent);
    }
    false
}

pub(crate) fn trace_label(world: &World, id: ComponentId) -> String {
    world.component_label(id).unwrap_or("<unnamed>").to_string()
}

pub(crate) fn fmt_opt(value: Option<f32>) -> String {
    value
        .map(|v| format!("{v:.6}"))
        .unwrap_or_else(|| "None".to_string())
}

/// Pass 1 — measure a single TC layout item.
///
/// Reads the [`StyleComponent`] from among `tc_id`'s ECS children and computes
/// the full box model (content, padding, margin) for both axes.
///
/// `unit_scale` is the nearest enclosing `LayoutComponent.unit_scale` —
/// used to convert `SizeDimension::WorldUnits(_)` values back to glyph units
/// for the GU-internal layout math, and to convert the renderer's wu glyph
/// scale back into GU for text intrinsic sizing.
pub(crate) fn measure_item(
    world: &World,
    tc_id: ComponentId,
    avail_w_gu: f32,
    avail_h_gu: Option<f32>,
    unit_scale: f32,
) -> MeasuredItem {
    // Copy style fields out before the borrow ends.
    let children: Vec<ComponentId> = world.children_of(tc_id).to_vec();
    let style = children.iter().find_map(|&child| {
        world
            .get_component_by_id_as::<StyleComponent>(child)
            .map(|s| {
                (
                    s.padding,
                    s.margin,
                    s.height,
                    s.width,
                    s.display,
                    s.flex_grow,
                    s.box_sizing,
                )
            })
    });

    let (padding, margin, height, width, style_display, _flex_grow, box_sizing) =
        style.unwrap_or_default();

    // Resolve display: StyleComponent.display overrides HtmlElementComponent UA default.
    let ua_display = children.iter().find_map(|&child| {
        world
            .get_component_by_id_as::<HtmlElementComponent>(child)
            .and_then(|el| el.element_type.default_display())
    });
    let display = style_display.or(ua_display);

    let is_block = matches!(display, None | Some(Display::Block));

    // Resolve padding/margin against the inline-axis container width (CSS semantic).
    let margin = margin.resolve(avail_w_gu, unit_scale);
    let padding = padding.resolve(avail_w_gu, unit_scale);

    // ── Horizontal ───────────────────────────────────────────────────────
    let margin_left_gu = margin.left;
    let margin_right_gu = margin.right;
    let padding_left_gu = padding.left;
    let padding_right_gu = padding.right;

    // Box-sizing: border-box (cat-engine default).
    // `width(...)` describes the OUTER (padding+border+content) box. Padding
    // eats into the content area. This differs from CSS's default
    // `content-box` but matches the modern best-practice / Bootstrap default
    // and makes percent math compose cleanly: two siblings with
    // `width(25%) + width(75%)` fit a parent's content width exactly even
    // when each has its own padding.
    //
    // Percent resolves against the *containing block's content width*
    // (`avail_w_gu` — passed in by the caller, already net of the parent's
    // own padding). Not against this item's own avail-minus-padding.
    let max_box_w_gu = (avail_w_gu - margin_left_gu - margin_right_gu).max(0.0);
    let avail_content_w_gu = (max_box_w_gu - padding_left_gu - padding_right_gu).max(0.0);
    let renderable_intrinsic_width = matches!(width, SizeDimension::Auto)
        .then(|| intrinsic_block_width(world, tc_id, display, avail_content_w_gu, unit_scale))
        .flatten();

    let is_auto_width = matches!(width, SizeDimension::Auto)
        && matches!(
            display,
            None | Some(Display::Block | Display::InlineBlock | Display::Inline)
        );
    let padding_h = padding_left_gu + padding_right_gu;
    // Resolve a `WorldUnits(_)` width / height to glyph units once at the top
    // of the match so the rest stays unit-agnostic.
    let width_gu = match width {
        SizeDimension::WorldUnits(v) if unit_scale.abs() > f32::EPSILON => {
            SizeDimension::GlyphUnits(v / unit_scale)
        }
        SizeDimension::WorldUnits(v) => SizeDimension::GlyphUnits(v),
        other => other,
    };
    let (content_width_gu, box_width_gu) = match (width_gu, box_sizing) {
        // Explicit length, border-box: width is the outer box; content shrinks for padding.
        (SizeDimension::GlyphUnits(w), BoxSizing::BorderBox) => {
            let box_w = w.min(max_box_w_gu);
            ((box_w - padding_h).max(0.0), box_w)
        }
        // Explicit length, content-box (CSS default): width is the content; padding adds outside.
        (SizeDimension::GlyphUnits(w), BoxSizing::ContentBox) => {
            let box_w = (w + padding_h).min(max_box_w_gu);
            ((box_w - padding_h).max(0.0), box_w)
        }
        // Percent, border-box: % resolves to the outer box width.
        (SizeDimension::Percent(p), BoxSizing::BorderBox) => {
            let box_w = (avail_w_gu * p / 100.0).min(max_box_w_gu);
            ((box_w - padding_h).max(0.0), box_w)
        }
        // Percent, content-box: % resolves to the content width.
        (SizeDimension::Percent(p), BoxSizing::ContentBox) => {
            let box_w = (avail_w_gu * p / 100.0 + padding_h).min(max_box_w_gu);
            ((box_w - padding_h).max(0.0), box_w)
        }
        // Auto width: independent of box-sizing — content from intrinsic/fill, box adds padding.
        (SizeDimension::Auto, _) => {
            let content = match renderable_intrinsic_width {
                Some(w) => w,
                None => avail_content_w_gu,
            };
            (content, padding_h + content)
        }
        // `WorldUnits` is normalised to `GlyphUnits` above — the arm is
        // unreachable but kept exhaustive for the compiler.
        (SizeDimension::WorldUnits(_), _) => (0.0, padding_h),
    };
    let margin_box_width_gu = margin_left_gu + box_width_gu + margin_right_gu;

    // ── Vertical ─────────────────────────────────────────────────────────
    let margin_top_gu = margin.top;
    let margin_bottom_gu = margin.bottom;
    let padding_top_gu = padding.top;
    let padding_bottom_gu = padding.bottom;

    let is_inline_block = matches!(display, Some(Display::InlineBlock | Display::Inline));
    let padding_v = padding_top_gu + padding_bottom_gu;
    let height_gu = match height {
        SizeDimension::WorldUnits(v) if unit_scale.abs() > f32::EPSILON => {
            SizeDimension::GlyphUnits(v / unit_scale)
        }
        SizeDimension::WorldUnits(v) => SizeDimension::GlyphUnits(v),
        other => other,
    };
    let (content_height_gu, box_height_gu) = match (height_gu, box_sizing) {
        (SizeDimension::GlyphUnits(h), BoxSizing::BorderBox) => ((h - padding_v).max(0.0), h),
        (SizeDimension::GlyphUnits(h), BoxSizing::ContentBox) => (h, h + padding_v),
        (SizeDimension::Percent(p), BoxSizing::BorderBox) => {
            let box_h = avail_h_gu.map(|h| h * p / 100.0).unwrap_or(0.0);
            ((box_h - padding_v).max(0.0), box_h)
        }
        (SizeDimension::Percent(p), BoxSizing::ContentBox) => {
            let content_h = avail_h_gu.map(|h| h * p / 100.0).unwrap_or(0.0);
            (content_h, content_h + padding_v)
        }
        (SizeDimension::Auto, _) if is_block || is_inline_block => {
            let c = intrinsic_block_height(world, tc_id, content_width_gu, unit_scale);
            (c, padding_v + c)
        }
        (SizeDimension::Auto, _) => (0.0, padding_v),
        // Unreachable — normalised above.
        (SizeDimension::WorldUnits(_), _) => (0.0, padding_v),
    };
    let margin_box_height_gu = margin_top_gu + box_height_gu + margin_bottom_gu;

    if trace_layout_id(world, tc_id) {
        println!(
            "[layout-trace] measure item={} id={:?} avail_gu=({:.6},{}) style_size=({:?},{:?}) padding_gu=({:.6},{:.6},{:.6},{:.6}) margin_gu=({:.6},{:.6},{:.6},{:.6}) content_gu=({:.6},{:.6}) box_gu=({:.6},{:.6}) margin_box_gu=({:.6},{:.6}) local_wu=({:.6},{:.6})",
            trace_label(world, tc_id),
            tc_id,
            avail_w_gu,
            fmt_opt(avail_h_gu),
            width,
            height,
            padding_top_gu,
            padding_right_gu,
            padding_bottom_gu,
            padding_left_gu,
            margin_top_gu,
            margin_right_gu,
            margin_bottom_gu,
            margin_left_gu,
            content_width_gu,
            content_height_gu,
            box_width_gu,
            box_height_gu,
            margin_box_width_gu,
            margin_box_height_gu,
            box_width_gu * unit_scale,
            box_height_gu * unit_scale,
        );
    }

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
    avail_h_gu: Option<f32>,
    unit_scale: f32,
) -> Vec<MeasuredItem> {
    let children: Vec<ComponentId> = world.children_of(container_id).to_vec();
    if trace_layout_id(world, container_id) {
        println!(
            "[layout-trace] measure_container_items container={} id={:?} children_count={}",
            trace_label(world, container_id),
            container_id,
            children.len()
        );
        for &child in &children {
            let label = world.component_label(child).unwrap_or("");
            let has_tc = world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some();
            let is_layout = is_layout_item(world, child);
            println!(
                "[layout-trace]   - child={:?} label={:?} has_tc={} is_layout={}",
                child, label, has_tc, is_layout
            );
        }
    }
    children
        .into_iter()
        .filter(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
                && !world
                    .component_label(child)
                    .map(|label| label.starts_with("__"))
                    .unwrap_or(false)
                && is_layout_item(world, child)
        })
        .map(|child| measure_item(world, child, avail_w_gu, avail_h_gu, unit_scale))
        .collect()
}

pub(crate) fn is_layout_item(world: &World, tc_id: ComponentId) -> bool {
    let mut has_layout = false;
    for &child in world.children_of(tc_id) {
        if world
            .get_component_by_id_as::<StyleComponent>(child)
            .is_some()
            || world
                .get_component_by_id_as::<HtmlElementComponent>(child)
                .is_some()
        {
            has_layout = true;
            break;
        }
    }
    has_layout
}

/// Pass 1 — measure all TC children of a [`LayoutComponent`] root.
///
/// Returns `(items, avail_w_gu, avail_h_gu, unit_scale)`.
///
pub(crate) fn measure_items(
    world: &World,
    layout_id: ComponentId,
) -> (Vec<MeasuredItem>, f32, Option<f32>, f32) {
    let (avail_w, avail_h, unit_scale) = layout_root_available_bounds(world, layout_id);

    let items = measure_container_items(world, layout_id, avail_w, avail_h, unit_scale);

    (items, avail_w, avail_h, unit_scale)
}

// ── Text intrinsic height ─────────────────────────────────────────────────────

/// Glyph character advance in glyph units. Must match the renderer's
/// per-char x advance in `TextSystem::register_text` (1.0 col per glyph).
const CHAR_WIDTH_GU: f32 = 1.0;

/// Convert a content-area width (in glyph units) into the maximum number of
/// columns that fit. Single source of truth — `apply_text_wrap_for_item`,
/// `text_intrinsic_height`, and `text_intrinsic_width` must agree, or the
/// inline cursor's measured box width will disagree with the wrap_at the
/// renderer later applies (text overflows or under-wraps).
///
/// Glyphs are column-centered: column `n` spans `[n - 0.5, n + 0.5]`. With
/// the text origin at the content-box left edge, the leftmost glyph (col 0)
/// extends to `-0.5` from the origin and the rightmost (col `N-1`) extends
/// to `N - 0.5`. So `N` glyphs occupy a horizontal span of `N` units; we
/// simply floor the content width.
fn container_cols_for_width(content_width_gu: f32) -> usize {
    (content_width_gu / CHAR_WIDTH_GU).floor().max(1.0) as usize
}

/// `font_size_wu` is the glyph advance in **world units** (the renderer-frame
/// scale a glyph quad is drawn at). `content_width_gu` is the content-box
/// width in glyph units. `unit_scale` converts between the two: 1 GU =
/// `unit_scale` wu. The column count is the floor of how many `font_size_wu`-
/// wide glyphs fit in `content_width_gu * unit_scale` world units.
fn container_cols_for_width_and_font_size(
    content_width_gu: f32,
    font_size_wu: f32,
    unit_scale: f32,
) -> usize {
    if unit_scale.abs() <= f32::EPSILON {
        // Identity fallback: no layout root scale in scope; treat font_size as GU.
        let glyph_advance_gu = font_size_wu.max(f32::EPSILON) * CHAR_WIDTH_GU;
        return (content_width_gu / glyph_advance_gu).floor().max(1.0) as usize;
    }
    let content_width_wu = content_width_gu * unit_scale;
    let glyph_advance_wu = font_size_wu.max(f32::EPSILON) * CHAR_WIDTH_GU;
    (content_width_wu / glyph_advance_wu).floor().max(1.0) as usize
}

/// Walk the subtree of `root` and return the `ComponentId` of the first
/// `TextComponent` found within local content (not crossing nested TCs).
pub(crate) fn find_text_id_in_local_content_subtree(
    world: &World,
    root: ComponentId,
) -> Option<ComponentId> {
    fn visit(world: &World, node: ComponentId, root: ComponentId) -> Option<ComponentId> {
        if world
            .get_component_by_id_as::<TextComponent>(node)
            .is_some()
        {
            return Some(node);
        }

        if node != root {
            if world
                .get_component_by_id_as::<LayoutComponent>(node)
                .is_some()
            {
                return None;
            }
            let is_boundary = world.children_of(node).iter().any(|&ch| {
                world.get_component_by_id_as::<StyleComponent>(ch).is_some()
                    || world
                        .get_component_by_id_as::<HtmlElementComponent>(ch)
                        .is_some()
            });
            if is_boundary {
                return None;
            }
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
///
/// Descends through plain `TransformComponent` wrappers (common pattern:
/// `T.position(…){ Text { … } }`) so a box can measure text wrapped in a
/// positioning inner T. Halts at nested *styled* layout items — those are
/// their own boxes and shouldn't bleed text up here.
fn find_text_in_local_content_subtree(
    world: &World,
    root: ComponentId,
) -> Option<(String, usize, bool, Vec<String>, f32)> {
    fn visit(
        world: &World,
        node: ComponentId,
        root: ComponentId,
    ) -> Option<(String, usize, bool, Vec<String>, f32)> {
        if let Some(t) = world.get_component_by_id_as::<TextComponent>(node) {
            // Return the *authored* wrap_at — callers use this as a hard cap
            // against the current container width. `t.wrap_at` reflects a prior
            // pass's already-narrowed value and would prevent re-widening when
            // the container grows.
            return Some((
                t.text.clone(),
                t.authored_wrap_at,
                t.word_wrap,
                t.word_wrap_tokens.clone(),
                t.font_size,
            ));
        }

        if node != root {
            if world
                .get_component_by_id_as::<LayoutComponent>(node)
                .is_some()
            {
                return None;
            }
            let is_boundary = world.children_of(node).iter().any(|&ch| {
                world.get_component_by_id_as::<StyleComponent>(ch).is_some()
                    || world
                        .get_component_by_id_as::<HtmlElementComponent>(ch)
                        .is_some()
            });
            if is_boundary {
                return None;
            }
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

/// Convert a length expressed in world units back into glyph units using the
/// nearest layout root's `unit_scale`. `unit_scale = 0.0` (no layout root in
/// scope, or a degenerate root) returns the input unchanged so callers can
/// reuse this in non-layout contexts without special-casing.
fn wu_to_gu(value_wu: f32, unit_scale: f32) -> f32 {
    if unit_scale.abs() > f32::EPSILON {
        value_wu / unit_scale
    } else {
        value_wu
    }
}

/// Resolve `Style.font_size` (a `SizeDimension`) to a **world-unit** glyph
/// scale, using `unit_scale` for the GU → WU conversion. Returns `None` when
/// the styled TC has no `StyleComponent`, or when its `font_size` is `Auto`
/// (which means "fall through to the descendant `TextComponent`'s authored
/// value"). Negative / zero values are also treated as "unset" — they would
/// produce invisible glyphs and almost always indicate a missed override.
fn resolved_style_font_size_wu(world: &World, tc_id: ComponentId, unit_scale: f32) -> Option<f32> {
    let style_size = world.children_of(tc_id).iter().find_map(|&child| {
        world
            .get_component_by_id_as::<StyleComponent>(child)
            .map(|s| s.font_size)
    })?;
    let wu = match style_size {
        SizeDimension::Auto => return None,
        SizeDimension::GlyphUnits(g) => g * unit_scale,
        SizeDimension::WorldUnits(w) => w,
        // `Percent` doesn't have a defined CSS meaning for `font-size` here;
        // treat as "unset" rather than guessing what reference value to use.
        SizeDimension::Percent(_) => return None,
    };
    (wu > 0.0).then_some(wu)
}

/// Measure the intrinsic block-axis height (in glyph units) of a TC subtree
/// by finding its `TextComponent` and running `TextSystem::measure`.
///
/// Returns `0.0` if no `TextComponent` is found in the subtree.
///
/// `TextSystem::measure` works in world units (it multiplies `rows * font_size_wu`),
/// since the renderer scales glyph quads by `font_size_wu` in the styled-TC's
/// local world frame. We convert back to GU via `unit_scale` so the layout
/// system stays GU-internal.
fn text_intrinsic_height(
    world: &World,
    tc_id: ComponentId,
    content_width_gu: f32,
    unit_scale: f32,
) -> f32 {
    let Some((text, existing_wrap_at, mut word_wrap, mut tokens, text_font_size_wu)) =
        find_text_in_local_content_subtree(world, tc_id)
    else {
        return 0.0;
    };
    let effective_font_size_wu =
        resolved_style_font_size_wu(world, tc_id, unit_scale).unwrap_or(text_font_size_wu);

    // Apply StyleComponent word_wrap override before measuring, the same way
    // apply_text_wrap_for_item does. This ensures the layout measurement
    // matches the renderer's wrapping behavior.
    let (style_word_wrap, style_tokens) = read_text_wrap_style(world, tc_id);
    match style_word_wrap {
        Some(WordWrapMode::Normal) => word_wrap = true,
        Some(WordWrapMode::BreakWord) | Some(WordWrapMode::BreakAll) => word_wrap = false,
        None => {}
    }
    if let Some(t) = style_tokens {
        tokens = t;
    }

    // Derive wrap_at from available width if the content area is known and wider
    // than a single character; otherwise fall back to the TextComponent's own wrap_at.
    // Use the container-derived wrap_at, but never exceed the TextComponent's own
    // wrap_at — the TextSystem will use that limit, so measuring with a larger value
    // would undercount lines for texts that hit the TC's hard-wrap point.
    // Glyph quads are centered at column positions, so the rightmost glyph
    // spans [col-0.5, col+0.5]. Reserve half a glyph on the right edge so the
    // last glyph's right half fits inside the content box (and inside padding).
    let wrap_at = if content_width_gu > CHAR_WIDTH_GU {
        let container_cols = container_cols_for_width_and_font_size(
            content_width_gu,
            effective_font_size_wu,
            unit_scale,
        );
        if existing_wrap_at == 0 {
            container_cols
        } else {
            container_cols.min(existing_wrap_at)
        }
    } else if existing_wrap_at == 0 {
        // No container width and no author cap — measure unwrapped.
        usize::MAX
    } else {
        existing_wrap_at
    };

    let (_width_wu, height_wu) = TextSystem::measure(
        &text,
        wrap_at.max(1),
        word_wrap,
        &tokens,
        effective_font_size_wu,
    );
    wu_to_gu(height_wu, unit_scale)
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
    unit_scale: f32,
) {
    let Some(text_id) = find_text_id_in_local_content_subtree(world, tc_id) else {
        return;
    };
    if content_width_gu <= CHAR_WIDTH_GU {
        return;
    }
    // Style overrides on the styled TC propagate onto the descendant TextComponent.
    // No cascade today — only this TC's own StyleComponent is consulted.
    let (style_word_wrap, style_tokens) = read_text_wrap_style(world, tc_id);

    let (
        cur_wrap_at,
        authored_wrap_at,
        cur_word_wrap,
        authored_word_wrap,
        cur_tokens,
        authored_word_wrap_tokens,
        cur_text,
        cur_font_size_wu,
    ) = match world.get_component_by_id_as::<TextComponent>(text_id) {
        Some(tc) => (
            tc.wrap_at,
            tc.authored_wrap_at,
            tc.word_wrap,
            tc.authored_word_wrap,
            tc.word_wrap_tokens.clone(),
            tc.authored_word_wrap_tokens.clone(),
            tc.text.clone(),
            tc.font_size,
        ),
        None => return,
    };
    let container_cols =
        container_cols_for_width_and_font_size(content_width_gu, cur_font_size_wu, unit_scale);

    // Cap against the *authored* wrap_at, not the current value (the current
    // value may have been narrowed by a prior layout pass at a smaller
    // container width — capping there would prevent re-widening).
    // An authored cap of `0` means "no author limit" — fill the container.
    let new_wrap_at = if authored_wrap_at == 0 {
        container_cols
    } else {
        container_cols.min(authored_wrap_at)
    };
    // CSS `overflow-wrap` semantics:
    //   `normal`     — only break at whitespace/token boundaries; long words
    //                  overflow rather than being split (TextComponent
    //                  `word_wrap = true`).
    //   `break-word` / `break-all` — break anywhere if needed to prevent overflow
    //                  (hard wrap at `wrap_at`; TextComponent `word_wrap = false`).
    let new_word_wrap = match style_word_wrap {
        Some(WordWrapMode::Normal) => true,
        Some(WordWrapMode::BreakWord) | Some(WordWrapMode::BreakAll) => false,
        None => authored_word_wrap,
    };
    let new_tokens = style_tokens.unwrap_or(authored_word_wrap_tokens);

    if new_wrap_at == cur_wrap_at && new_word_wrap == cur_word_wrap && new_tokens == cur_tokens {
        return;
    }

    if let Some(tc) = world.get_component_by_id_as_mut::<TextComponent>(text_id) {
        tc.wrap_at = new_wrap_at;
        tc.word_wrap = new_word_wrap;
        tc.word_wrap_tokens = new_tokens;
    }
    emit.push_intent_now(
        text_id,
        crate::engine::ecs::IntentValue::SetText {
            component_ids: vec![text_id],
            text: cur_text,
        },
    );
}

pub(crate) fn apply_text_font_size_for_item(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    tc_id: ComponentId,
    unit_scale: f32,
) {
    // `Style.font_size` carries a unit (`SizeDimension`): `GlyphUnits(g)`
    // means "g glyph units per row" → wu via `g * unit_scale`; `WorldUnits(w)`
    // means "w world units per row" → wu directly; `Auto` defers to the
    // descendant `TextComponent`'s authored value. `Percent` has no defined
    // reference here and is treated as `Auto`.
    let style_font_size_wu = resolved_style_font_size_wu(world, tc_id, unit_scale);

    let Some(text_id) = find_text_id_in_local_content_subtree(world, tc_id) else {
        return;
    };

    let (cur_font_size, authored_font_size, cur_text) =
        match world.get_component_by_id_as::<TextComponent>(text_id) {
            Some(tc) => (tc.font_size, tc.authored_font_size, tc.text.clone()),
            None => return,
        };

    let new_font_size = style_font_size_wu.unwrap_or(authored_font_size);

    if (new_font_size - cur_font_size).abs() <= f32::EPSILON {
        return;
    }

    if let Some(tc) = world.get_component_by_id_as_mut::<TextComponent>(text_id) {
        tc.set_effective_font_size(new_font_size);
    }

    emit.push_intent_now(
        text_id,
        crate::engine::ecs::IntentValue::SetText {
            component_ids: vec![text_id],
            text: cur_text,
        },
    );
}

/// Layout-owned `ColorComponent` child label. Spawned/maintained by
/// `apply_text_color_for_item` whenever the styled TC has `Style.color = Some(_)`.
/// Sits as an immediate child of the styled TC (sibling of `__bg`) so the
/// renderable ancestor walk picks it up for every glyph in the subtree.
const OWNED_TEXT_COLOR_LABEL: &str = "__text_color";

/// Spawn / update / remove the layout-owned `__text_color` helper based on
/// `Style.color` of the styled TC. Cascade is provided by the renderable
/// system's ancestor color walk
/// (`RenderableSystem::inherited_color_for_renderable`) — no per-glyph
/// attachment needed, and nested styled TCs override naturally because their
/// helper sits closer to the glyph in the walk.
pub(crate) fn apply_text_color_for_item(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    tc_id: ComponentId,
) {
    let style_color = world
        .children_of(tc_id)
        .iter()
        .find_map(|&child| {
            world
                .get_component_by_id_as::<StyleComponent>(child)
                .map(|s| s.color)
        })
        .flatten();

    let existing = world.children_of(tc_id).iter().copied().find(|&ch| {
        world.component_label(ch) == Some(OWNED_TEXT_COLOR_LABEL)
            && world.get_component_by_id_as::<ColorComponent>(ch).is_some()
    });

    match (style_color, existing) {
        (Some(rgba), Some(id)) => {
            // Update if rgba changed; re-register so the renderable picks it up.
            let cur = world
                .get_component_by_id_as::<ColorComponent>(id)
                .map(|c| c.rgba);
            if cur != Some(rgba) {
                if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(id) {
                    c.rgba = rgba;
                }
                emit.push_intent_now(
                    id,
                    crate::engine::ecs::IntentValue::RegisterColor {
                        component_ids: vec![id],
                    },
                );
            }
        }
        (Some(rgba), None) => {
            // Spawn unconditionally. `find_text_id_in_local_content_subtree`
            // halts at nested TransformComponent boundaries, so it can't see
            // text living past styled-child layout items; using it as a gate
            // would suppress the helper exactly where cascade is wanted.
            // A helper with no descendant text is harmless — the renderable
            // ancestor walk just never hits it.
            let color_id = world.add_component_boxed_named(
                OWNED_TEXT_COLOR_LABEL,
                Box::new(ColorComponent::rgba(rgba[0], rgba[1], rgba[2], rgba[3])),
            );
            let _ = world.add_child(tc_id, color_id);
            world.init_component_tree(color_id, emit);
        }
        (None, Some(id)) => {
            emit.push_intent_now(
                id,
                crate::engine::ecs::IntentValue::RemoveSubtree {
                    component_ids: vec![id],
                },
            );
        }
        (None, None) => {}
    }
}

/// Pull text-wrap overrides off the `StyleComponent` sitting under `tc_id`,
/// if any. Returns `(word_wrap_mode, tokens)` — both `None` when nothing is
/// styled (so the TextComponent's authored values stand).
fn read_text_wrap_style(
    world: &World,
    tc_id: ComponentId,
) -> (Option<WordWrapMode>, Option<Vec<String>>) {
    for &child in world.children_of(tc_id) {
        if let Some(st) = world.get_component_by_id_as::<StyleComponent>(child) {
            return (st.word_wrap, st.word_wrap_tokens.clone());
        }
    }
    (None, None)
}

/// Walk the local-content subtree of `root` and union the `BoundsComponent.local`
/// of every direct `RenderableComponent` found. Stops at nested TransformComponents
/// (same rule as `find_text_in_local_content_subtree`) so a TC only sees its own
/// renderable children, not those belonging to descendant TCs.
pub(crate) fn find_renderable_local_bounds(world: &World, root: ComponentId) -> Option<Aabb> {
    fn visit(world: &World, node: ComponentId, root: ComponentId, acc: &mut Option<Aabb>) {
        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            // Look for an attached BoundsComponent among this renderable's children.
            for &c in world.children_of(node) {
                if let Some(b) = world.get_component_by_id_as::<BoundsComponent>(c) {
                    *acc = Some(match acc {
                        Some(prev) => prev.union(&b.local),
                        None => b.local,
                    });
                    break;
                }
            }
        }
        if node != root
            && world
                .get_component_by_id_as::<TransformComponent>(node)
                .is_some()
        {
            return;
        }
        for &child in world.children_of(node) {
            visit(world, child, root, acc);
        }
    }
    let mut acc = None;
    visit(world, root, root, &mut acc);
    acc
}

/// Intrinsic inline-axis width for a TC whose width style is `Auto`.
///
/// Returns `Some(width)` when the TC has a direct renderable child whose
/// bounds can size the container (shrink-to-fit). Returns `None` for text
/// cells or layout containers — those should keep filling the available
/// inline budget so text wraps inside them.
pub(crate) fn intrinsic_block_width(
    world: &World,
    tc_id: ComponentId,
    display: Option<Display>,
    avail_content_w_gu: f32,
    unit_scale: f32,
) -> Option<f32> {
    let is_inline_block = matches!(display, Some(Display::InlineBlock | Display::Inline));
    if find_text_in_local_content_subtree(world, tc_id).is_some() {
        // CSS-aligned: inline-block shrinks to fit its content; block fills
        // the available inline budget so text wraps inside it.
        if is_inline_block {
            return Some(text_intrinsic_width(
                world,
                tc_id,
                avail_content_w_gu,
                unit_scale,
            ));
        }
        return None;
    }
    find_renderable_local_bounds(world, tc_id).map(|a| a.width())
}

/// Width (in glyph units) of the widest line of the descendant
/// `TextComponent`, measured wrapped at `avail_content_w_gu` columns. CSS
/// inline-block shrink-to-fit: cap to the containing block's available
/// width, then take the widest resulting line. Pairs with
/// `text_intrinsic_height`.
fn text_intrinsic_width(
    world: &World,
    tc_id: ComponentId,
    avail_content_w_gu: f32,
    unit_scale: f32,
) -> f32 {
    let Some((text, tc_wrap_at, word_wrap, tokens, text_font_size_wu)) =
        find_text_in_local_content_subtree(world, tc_id)
    else {
        return 0.0;
    };
    let effective_font_size_wu =
        resolved_style_font_size_wu(world, tc_id, unit_scale).unwrap_or(text_font_size_wu);
    let avail_cols = if avail_content_w_gu > CHAR_WIDTH_GU {
        container_cols_for_width_and_font_size(
            avail_content_w_gu,
            effective_font_size_wu,
            unit_scale,
        )
    } else {
        0
    };
    // Honor the TextComponent's authored wrap_at as a hard cap (0 = unlimited).
    let wrap_at = match (avail_cols, tc_wrap_at) {
        (0, 0) => usize::MAX,
        (0, w) => w,
        (a, 0) => a,
        (a, w) => a.min(w),
    };
    let (measured_wu, _height_wu) =
        TextSystem::measure(&text, wrap_at, word_wrap, &tokens, effective_font_size_wu);
    let measured_gu = wu_to_gu(measured_wu, unit_scale);
    // CSS shrink-to-fit caps an inline-block's width at the available content
    // width even if the text inside overflows (word-wrap: normal with a long
    // unbreakable token). Without this cap, the box claims a width wider than
    // its containing block and downstream measurements (wrap_at on glyph
    // rebuild, sibling line-break decisions) inherit the inflated value.
    if avail_content_w_gu > 0.0 {
        measured_gu.min(avail_content_w_gu)
    } else {
        measured_gu
    }
}

fn intrinsic_block_height(
    world: &World,
    tc_id: ComponentId,
    content_width_gu: f32,
    unit_scale: f32,
) -> f32 {
    if find_text_in_local_content_subtree(world, tc_id).is_some() {
        return text_intrinsic_height(world, tc_id, content_width_gu, unit_scale);
    }

    if let Some(aabb) = find_renderable_local_bounds(world, tc_id) {
        return aabb.height();
    }

    let child_items = measure_container_items(world, tc_id, content_width_gu, None, unit_scale);
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
        return child_items
            .iter()
            .map(|item| item.margin_box_height_gu)
            .sum();
    }

    descendant_layout_intrinsic_height(world, tc_id).unwrap_or(0.0)
}

fn descendant_layout_intrinsic_height(world: &World, root: ComponentId) -> Option<f32> {
    let mut total_height = 0.0;
    let mut found_height = false;

    for &child in world.children_of(root) {
        if world
            .get_component_by_id_as::<LayoutComponent>(child)
            .is_some()
        {
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

use crate::engine::ecs::component::style::{BoxSizing, EdgeInsets};

type StyleTuple = (
    EdgeInsets,
    EdgeInsets,
    SizeDimension,
    SizeDimension,
    Option<Display>,
    f32,
    BoxSizing,
);

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
            BoxSizing::BorderBox,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{measure_container_items, measure_item, measure_items};
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::style::{Display, SizeDimension};
    use crate::engine::ecs::component::{
        ColorComponent, LayoutComponent, StyleComponent, TextComponent, TransformComponent,
    };

    #[test]
    fn auto_height_container_does_not_measure_text_behind_nested_transforms() {
        let mut world = World::default();

        let container =
            world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let style =
            world.add_component_boxed_named("content_style", Box::new(StyleComponent::new()));
        let panel =
            world.add_component_boxed_named("world_panel", Box::new(LayoutComponent::new(10.0)));
        let rows_track =
            world.add_component_boxed_named("rows_track", Box::new(TransformComponent::new()));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let text =
            world.add_component_boxed_named("row_text", Box::new(TextComponent::new("hello")));

        let _ = world.add_child(container, style);
        let _ = world.add_child(container, panel);
        let _ = world.add_child(panel, rows_track);
        let _ = world.add_child(rows_track, row);
        let _ = world.add_child(row, color);
        let _ = world.add_child(color, text);

        let measured = measure_item(&world, container, 29.5, None, 1.0);
        assert_eq!(measured.content_height_gu, 0.0);
    }

    #[test]
    fn percent_height_uses_known_container_height() {
        let mut world = World::default();

        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component_boxed_named(
            "row_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::Percent(25.0);
                s
            }),
        );

        let _ = world.add_child(row, row_style);

        let measured = measure_item(&world, row, 20.0, Some(24.0), 1.0);
        assert_eq!(measured.content_height_gu, 6.0);
        assert_eq!(measured.box_height_gu, 6.0);
    }

    #[test]
    fn layoutroot_world_units_resolve_against_ancestor_transform_scale() {
        let mut world = World::default();

        let panel = world.add_component_boxed_named(
            "panel",
            Box::new(TransformComponent::new().with_scale(0.1, 0.1, 0.1)),
        );
        let root = world.add_component(LayoutComponent::new(80.0).with_unit_scale(1.0));
        world
            .get_component_by_id_as_mut::<LayoutComponent>(root)
            .unwrap()
            .set_available_width_dimension(SizeDimension::WorldUnits(1.0));
        world
            .get_component_by_id_as_mut::<LayoutComponent>(root)
            .unwrap()
            .set_available_height_dimension(SizeDimension::WorldUnits(2.851));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component_boxed_named(
            "row_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::Percent(25.0);
                s
            }),
        );

        let _ = world.add_child(panel, root);
        let _ = world.add_child(root, row);
        let _ = world.add_child(row, row_style);

        let (items, avail_w, avail_h, _) = measure_items(&world, root);
        assert!(
            (avail_w - 10.0).abs() < 1e-4,
            "expected 1wu / 0.1scale => 10gu, got {avail_w}"
        );
        assert!(
            (avail_h.unwrap_or_default() - 28.51).abs() < 1e-3,
            "expected 2.851wu / 0.1scale => 28.51gu, got {:?}",
            avail_h
        );
        assert_eq!(items.len(), 1);
        assert!(
            (items[0].content_height_gu - 7.1275).abs() < 1e-3,
            "expected 25% of 28.51gu, got {}",
            items[0].content_height_gu
        );
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
        let text =
            world.add_component_boxed_named("row_text", Box::new(TextComponent::new("hello")));

        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, color);
        let _ = world.add_child(color, text);

        let measured = measure_item(&world, row, 12.0, None, 1.0);
        assert_eq!(measured.content_height_gu, 1.0);
    }

    #[test]
    fn block_auto_height_uses_intrinsic_child_item_height_instead_of_sharing_remaining_space() {
        let mut world = World::default();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(12.0));

        let container =
            world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let container_style =
            world.add_component_boxed_named("content_style", Box::new(StyleComponent::new()));

        let child = world.add_component_boxed_named("child", Box::new(TransformComponent::new()));
        let child_style = world.add_component_boxed_named(
            "child_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.height = SizeDimension::GlyphUnits(3.0);
                s
            }),
        );

        let sibling =
            world.add_component_boxed_named("sibling", Box::new(TransformComponent::new()));
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

        let container =
            world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let container_style =
            world.add_component_boxed_named("content_style", Box::new(StyleComponent::new()));
        let panel = world.add_component_boxed_named(
            "world_panel",
            Box::new(ColorComponent::rgba(0.0, 0.0, 0.0, 0.0)),
        );
        let scroll = world.add_component_boxed_named(
            "world_panel_scroll",
            Box::new(ColorComponent::rgba(0.0, 0.0, 0.0, 0.0)),
        );
        let rows_track =
            world.add_component_boxed_named("rows_track", Box::new(TransformComponent::new()));
        let rows_layout =
            world.add_component_boxed_named("rows_layout", Box::new(LayoutComponent::new(4.0)));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style =
            world.add_component_boxed_named("row_style", Box::new(StyleComponent::new()));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let text = world.add_component_boxed_named(
            "row_text",
            Box::new(TextComponent::new("hello world hello world")),
        );

        let _ = world.add_child(container, container_style);
        let _ = world.add_child(container, panel);
        let _ = world.add_child(panel, scroll);
        let _ = world.add_child(scroll, rows_track);
        let _ = world.add_child(rows_track, rows_layout);
        let _ = world.add_child(rows_layout, row);
        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, color);
        let _ = world.add_child(color, text);

        let measured = measure_item(&world, container, 4.0, None, 1.0);
        assert!(
            measured.content_height_gu > 1.0,
            "wrapped descendant layout text should increase intrinsic height"
        );
    }

    #[test]
    fn inline_block_with_text_shrinks_to_fit_when_width_auto() {
        let mut world = World::default();

        let tc = world.add_component_boxed_named("box", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "box_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::InlineBlock);
                s
            }),
        );
        let text = world.add_component_boxed_named("box_text", Box::new(TextComponent::new("abc")));

        let _ = world.add_child(tc, style);
        let _ = world.add_child(tc, text);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.content_width_gu, 3.0);
    }

    #[test]
    fn block_with_text_still_fills_available_width() {
        let mut world = World::default();

        let tc = world.add_component_boxed_named("box", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "box_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s
            }),
        );
        let text = world.add_component_boxed_named("box_text", Box::new(TextComponent::new("abc")));

        let _ = world.add_child(tc, style);
        let _ = world.add_child(tc, text);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.content_width_gu, 40.0);
    }

    #[test]
    fn inline_block_with_explicit_width_keeps_that_width() {
        let mut world = World::default();

        let tc = world.add_component_boxed_named("box", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "box_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::InlineBlock);
                s.width = SizeDimension::GlyphUnits(12.0);
                s
            }),
        );
        let text = world.add_component_boxed_named("box_text", Box::new(TextComponent::new("ab")));

        let _ = world.add_child(tc, style);
        let _ = world.add_child(tc, text);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.content_width_gu, 12.0);
    }

    #[test]
    fn text_wrap_relaxes_when_container_grows_back() {
        use super::apply_text_wrap_for_item;
        use crate::engine::ecs::ComponentId;
        use crate::engine::ecs::SignalEmitter;
        use crate::engine::ecs::rx::{EventSignal, IntentSignal};

        struct NullEmit;
        impl SignalEmitter for NullEmit {
            fn push_event(&mut self, _: ComponentId, _: EventSignal) {}
            fn push_intent(&mut self, _: ComponentId, _: IntentSignal) {}
        }

        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let text = world.add_component_boxed_named(
            "txt",
            Box::new(TextComponent::with_word_wrap(
                "the quick brown fox jumps over the lazy dog",
                60,
            )),
        );
        let _ = world.add_child(tc, text);

        let mut emit = NullEmit;

        // 1st pass: narrow container forces wrap_at down.
        apply_text_wrap_for_item(&mut world, &mut emit, tc, 10.0, 1.0);
        let narrow = world
            .get_component_by_id_as::<TextComponent>(text)
            .unwrap()
            .wrap_at;
        assert!(
            narrow < 60,
            "narrow container should reduce wrap_at, got {}",
            narrow
        );

        // 2nd pass: container grows back; wrap_at must widen toward the authored cap.
        apply_text_wrap_for_item(&mut world, &mut emit, tc, 80.0, 1.0);
        let wide = world
            .get_component_by_id_as::<TextComponent>(text)
            .unwrap()
            .wrap_at;
        assert!(
            wide > narrow,
            "wide container should re-widen wrap_at, got {} (was {})",
            wide,
            narrow
        );
        assert!(
            wide <= 60,
            "wrap_at must never exceed authored cap (60), got {}",
            wide
        );
    }

    #[test]
    fn style_font_size_overrides_descendant_text_font_size() {
        use super::apply_text_font_size_for_item;
        use crate::engine::ecs::ComponentId;
        use crate::engine::ecs::SignalEmitter;
        use crate::engine::ecs::rx::{EventSignal, IntentSignal};

        struct NullEmit;
        impl SignalEmitter for NullEmit {
            fn push_event(&mut self, _: ComponentId, _: EventSignal) {}
            fn push_intent(&mut self, _: ComponentId, _: IntentSignal) {}
        }

        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.font_size = SizeDimension::WorldUnits(0.25);
                s
            }),
        );
        let text = world.add_component_boxed_named(
            "txt",
            Box::new(TextComponent::new("hello").with_font_size(1.0)),
        );
        let _ = world.add_child(tc, style);
        let _ = world.add_child(tc, text);

        let mut emit = NullEmit;
        apply_text_font_size_for_item(&mut world, &mut emit, tc, 1.0);

        let effective = world
            .get_component_by_id_as::<TextComponent>(text)
            .unwrap()
            .font_size;
        let authored = world
            .get_component_by_id_as::<TextComponent>(text)
            .unwrap()
            .authored_font_size;
        assert!((effective - 0.25).abs() < 1e-6);
        assert!((authored - 1.0).abs() < 1e-6);
    }

    #[test]
    fn apply_text_wrap_descends_through_plain_transform_wrapper() {
        use super::apply_text_wrap_for_item;
        use crate::engine::ecs::ComponentId;
        use crate::engine::ecs::SignalEmitter;
        use crate::engine::ecs::rx::{EventSignal, IntentSignal};

        struct NullEmit;
        impl SignalEmitter for NullEmit {
            fn push_event(&mut self, _: ComponentId, _: EventSignal) {}
            fn push_intent(&mut self, _: ComponentId, _: IntentSignal) {}
        }

        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::InlineBlock);
                s.width = SizeDimension::GlyphUnits(8.0);
                s
            }),
        );
        let inner = world.add_component_boxed_named("inner", Box::new(TransformComponent::new()));
        let text = world.add_component_boxed_named(
            "txt",
            Box::new(
                TextComponent::with_word_wrap("inline 1.6 inline 1.6", 60).with_font_size(1.6),
            ),
        );
        let _ = world.add_child(tc, style);
        let _ = world.add_child(tc, inner);
        let _ = world.add_child(inner, text);

        let mut emit = NullEmit;
        apply_text_wrap_for_item(&mut world, &mut emit, tc, 8.0, 1.0);

        let wrap_at = world
            .get_component_by_id_as::<TextComponent>(text)
            .unwrap()
            .wrap_at;
        assert!(
            wrap_at < 60,
            "expected wrap_at to narrow through inner transform wrapper, got {wrap_at}"
        );
    }

    #[test]
    fn content_box_explicit_width_with_padding_grows_outer_box() {
        use crate::engine::ecs::component::style::{BoxSizing, EdgeInsets};
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s.box_sizing = BoxSizing::ContentBox;
                s.width = SizeDimension::GlyphUnits(20.0);
                s.padding = EdgeInsets::all(2.0);
                s
            }),
        );
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(
            measured.content_width_gu, 20.0,
            "content stays at width(20) under content-box"
        );
        assert_eq!(
            measured.box_width_gu, 24.0,
            "outer box = content + 2*padding"
        );
    }

    #[test]
    fn border_box_explicit_width_with_padding_keeps_outer_box_width() {
        use crate::engine::ecs::component::style::EdgeInsets;
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s.width = SizeDimension::GlyphUnits(20.0);
                s.padding = EdgeInsets::all(2.0);
                s
            }),
        );
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.box_width_gu, 20.0, "outer box stays at width(20)");
        assert_eq!(
            measured.content_width_gu, 16.0,
            "content shrinks for padding"
        );
    }

    #[test]
    fn border_box_percent_siblings_sum_to_parent_width_with_padding() {
        use crate::engine::ecs::component::style::EdgeInsets;
        let mut world = World::default();

        let root = world.add_component(LayoutComponent::new(80.0));

        let mk = |world: &mut World, name: &'static str, pct: f32| {
            let tc = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
            let style = world.add_component_boxed_named(
                "style",
                Box::new({
                    let mut s = StyleComponent::new();
                    s.display = Some(Display::InlineBlock);
                    s.width = SizeDimension::Percent(pct);
                    s.padding = EdgeInsets::all_dim(SizeDimension::Percent(2.0));
                    s
                }),
            );
            let _ = world.add_child(tc, style);
            tc
        };
        let a = mk(&mut world, "a", 25.0);
        let b = mk(&mut world, "b", 75.0);
        let _ = world.add_child(root, a);
        let _ = world.add_child(root, b);

        let (items, _, _, _) = measure_items(&world, root);
        assert_eq!(items.len(), 2);
        // box widths must sum to exactly 80gu so the inline cursor lays them
        // side-by-side without wrapping.
        assert!(
            (items[0].margin_box_width_gu + items[1].margin_box_width_gu - 80.0).abs() < 1e-4,
            "got {} + {} = {}, expected 80",
            items[0].margin_box_width_gu,
            items[1].margin_box_width_gu,
            items[0].margin_box_width_gu + items[1].margin_box_width_gu
        );
        assert!((items[0].margin_box_width_gu - 20.0).abs() < 1e-4);
        assert!((items[1].margin_box_width_gu - 60.0).abs() < 1e-4);
    }

    #[test]
    fn explicit_inline_panel_children_clamp_to_layoutroot_width_when_layoutroot_shrinks() {
        let mut world = World::default();

        let root = world.add_component(LayoutComponent::new(29.5));

        let make_inline_box = |world: &mut World, name: &'static str, width_gu: f32| {
            let tc = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
            let style = world.add_component_boxed_named(
                "style",
                Box::new({
                    let mut s = StyleComponent::new();
                    s.display = Some(Display::InlineBlock);
                    s.width = SizeDimension::GlyphUnits(width_gu);
                    s
                }),
            );
            let _ = world.add_child(tc, style);
            tc
        };

        let title = make_inline_box(&mut world, "title", 14.5);
        let save = make_inline_box(&mut world, "save", 6.875);
        let load = make_inline_box(&mut world, "load", 6.875);
        let _ = world.add_child(root, title);
        let _ = world.add_child(root, save);
        let _ = world.add_child(root, load);

        let (wide_items, wide_avail, _, _) = measure_items(&world, root);
        assert_eq!(wide_items.len(), 3);
        assert_eq!(wide_avail, 29.5);
        assert!((wide_items[0].margin_box_width_gu - 14.5).abs() < 1e-4);
        assert!((wide_items[1].margin_box_width_gu - 6.875).abs() < 1e-4);
        assert!((wide_items[2].margin_box_width_gu - 6.875).abs() < 1e-4);

        world
            .get_component_by_id_as_mut::<LayoutComponent>(root)
            .unwrap()
            .set_available_width(9.5);

        let (narrow_items, narrow_avail, _, _) = measure_items(&world, root);

        assert_eq!(narrow_items.len(), 3);
        assert_eq!(narrow_avail, 9.5);
        assert!((narrow_items[0].margin_box_width_gu - narrow_avail).abs() < 1e-4);
        assert!(
            (narrow_items[1].margin_box_width_gu - wide_items[1].margin_box_width_gu).abs() < 1e-4,
            "save button should keep its authored width when already within the narrow root budget"
        );
        assert!(
            (narrow_items[2].margin_box_width_gu - wide_items[2].margin_box_width_gu).abs() < 1e-4,
            "load button should keep its authored width when already within the narrow root budget"
        );
        assert!(
            narrow_items
                .iter()
                .all(|item| item.margin_box_width_gu <= narrow_avail + 1e-4)
        );
    }

    #[test]
    fn width_percent_resolves_against_available_content_width() {
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s.width = SizeDimension::Percent(50.0);
                s
            }),
        );
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.content_width_gu, 20.0);
    }

    #[test]
    fn layoutroot_percent_width_child_tracks_root_available_width() {
        let mut world = World::default();

        let root = world.add_component(LayoutComponent::new(10.0));

        let panel = world.add_component_boxed_named("panel", Box::new(TransformComponent::new()));
        let panel_style = world.add_component_boxed_named(
            "panel_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s.width = SizeDimension::Percent(100.0);
                s.padding = crate::engine::ecs::component::style::EdgeInsets::all(0.8);
                s
            }),
        );
        let _ = world.add_child(panel, panel_style);

        let section =
            world.add_component_boxed_named("section", Box::new(TransformComponent::new()));
        let section_style = world.add_component_boxed_named(
            "section_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s
            }),
        );
        let _ = world.add_child(section, section_style);
        let _ = world.add_child(panel, section);
        let _ = world.add_child(root, panel);

        let (items, avail_w, _, _) = measure_items(&world, root);
        assert_eq!(avail_w, 10.0);
        assert_eq!(items.len(), 1);
        assert!((items[0].box_width_gu - 10.0).abs() < 1e-4);
        assert!((items[0].content_width_gu - 8.4).abs() < 1e-4);

        let nested = measure_container_items(&world, panel, items[0].content_width_gu, None, 1.0);
        assert_eq!(nested.len(), 1);
        assert!((nested[0].box_width_gu - 8.4).abs() < 1e-4);

        world
            .get_component_by_id_as_mut::<LayoutComponent>(root)
            .unwrap()
            .set_available_width(6.0);

        let (narrow_items, narrow_avail, _, _) = measure_items(&world, root);
        assert_eq!(narrow_avail, 6.0);
        assert_eq!(narrow_items.len(), 1);
        assert!((narrow_items[0].box_width_gu - 6.0).abs() < 1e-4);
        assert!((narrow_items[0].content_width_gu - 4.4).abs() < 1e-4);

        let narrow_nested =
            measure_container_items(&world, panel, narrow_items[0].content_width_gu, None, 1.0);
        assert_eq!(narrow_nested.len(), 1);
        assert!((narrow_nested[0].box_width_gu - 4.4).abs() < 1e-4);
    }

    #[test]
    fn padding_percent_resolves_against_inline_axis_width() {
        use crate::engine::ecs::component::style::EdgeInsets;
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s.width = SizeDimension::GlyphUnits(20.0);
                s.padding = EdgeInsets::all_dim(SizeDimension::Percent(10.0));
                s
            }),
        );
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.padding_left_gu, 4.0);
        assert_eq!(measured.padding_right_gu, 4.0);
        assert_eq!(measured.padding_top_gu, 4.0);
        assert_eq!(measured.padding_bottom_gu, 4.0);
    }

    #[test]
    fn bare_number_width_setter_defaults_to_glyph_units_via_mms() {
        // Smoke check at the type level: SizeDimension::GlyphUnits is the
        // default produced by `arg_size_dimension` for bare Value::Number.
        // The Style setter writes that into `width` unchanged.
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(Display::Block);
                s.width = SizeDimension::GlyphUnits(20.0);
                s
            }),
        );
        let _ = world.add_child(tc, style);
        let measured = measure_item(&world, tc, 40.0, None, 1.0);
        assert_eq!(measured.content_width_gu, 20.0);
    }
}
