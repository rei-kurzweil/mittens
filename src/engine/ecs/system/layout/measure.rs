use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{BoundsComponent, HtmlElementComponent, LayoutComponent, RenderableComponent, StyleComponent, TextComponent, TransformComponent};
use crate::engine::ecs::component::style::{Display, SizeDimension, WordWrapMode};
use crate::engine::ecs::system::text_system::TextSystem;
use crate::engine::graphics::bounds::Aabb;

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
            (s.padding, s.margin, s.height, s.width, s.display, s.flex_grow, s.box_sizing)
        })
    });

    let (padding, margin, height, width, style_display, _flex_grow, box_sizing) = style.unwrap_or_default();

    // Resolve display: StyleComponent.display overrides HtmlElementComponent UA default.
    let ua_display = children.iter().find_map(|&child| {
        world.get_component_by_id_as::<HtmlElementComponent>(child)
            .and_then(|el| el.element_type.default_display())
    });
    let display = style_display.or(ua_display);

    let is_block = matches!(display, None | Some(Display::Block));

    // Resolve padding/margin against the inline-axis container width (CSS semantic).
    let margin = margin.resolve(avail_w_gu);
    let padding = padding.resolve(avail_w_gu);

    // ── Horizontal ───────────────────────────────────────────────────────
    let margin_left_gu   = margin.left;
    let margin_right_gu  = margin.right;
    let padding_left_gu  = padding.left;
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
    let avail_content_w_gu = (avail_w_gu - margin_left_gu - margin_right_gu
                                          - padding_left_gu - padding_right_gu).max(0.0);
    let renderable_intrinsic_width = matches!(width, SizeDimension::Auto)
        .then(|| intrinsic_block_width(world, tc_id, display, avail_content_w_gu))
        .flatten();

    let is_auto_width = matches!(width, SizeDimension::Auto)
        && matches!(display, None | Some(Display::Block | Display::InlineBlock | Display::Inline));
    let padding_h = padding_left_gu + padding_right_gu;
    let (content_width_gu, box_width_gu) = match (width, box_sizing) {
        // Explicit length, border-box: width is the outer box; content shrinks for padding.
        (SizeDimension::GlyphUnits(w), BoxSizing::BorderBox) => {
            ((w - padding_h).max(0.0), w)
        }
        // Explicit length, content-box (CSS default): width is the content; padding adds outside.
        (SizeDimension::GlyphUnits(w), BoxSizing::ContentBox) => {
            (w, w + padding_h)
        }
        // Percent, border-box: % resolves to the outer box width.
        (SizeDimension::Percent(p), BoxSizing::BorderBox) => {
            let box_w = avail_w_gu * p / 100.0;
            ((box_w - padding_h).max(0.0), box_w)
        }
        // Percent, content-box: % resolves to the content width.
        (SizeDimension::Percent(p), BoxSizing::ContentBox) => {
            let c = avail_w_gu * p / 100.0;
            (c, c + padding_h)
        }
        // Auto width: independent of box-sizing — content from intrinsic/fill, box adds padding.
        (SizeDimension::Auto, _) => {
            let content = match renderable_intrinsic_width {
                Some(w) => w,
                None => avail_content_w_gu,
            };
            (content, padding_h + content)
        }
    };
    let margin_box_width_gu = margin_left_gu + box_width_gu + margin_right_gu;

    // ── Vertical ─────────────────────────────────────────────────────────
    let margin_top_gu    = margin.top;
    let margin_bottom_gu = margin.bottom;
    let padding_top_gu   = padding.top;
    let padding_bottom_gu = padding.bottom;

    let is_inline_block = matches!(display, Some(Display::InlineBlock | Display::Inline));
    let padding_v = padding_top_gu + padding_bottom_gu;
    let (content_height_gu, box_height_gu) = match (height, box_sizing) {
        (SizeDimension::GlyphUnits(h), BoxSizing::BorderBox) => {
            ((h - padding_v).max(0.0), h)
        }
        (SizeDimension::GlyphUnits(h), BoxSizing::ContentBox) => {
            (h, h + padding_v)
        }
        // Percent height with unknown container height falls back to 0.0 (matches
        // CSS conservative behavior for percent heights with auto parents).
        (SizeDimension::Percent(_), _) => (0.0, padding_v),
        (SizeDimension::Auto, _) if is_block || is_inline_block => {
            let c = intrinsic_block_height(world, tc_id, content_width_gu);
            (c, padding_v + c)
        }
        (SizeDimension::Auto, _) => (0.0, padding_v),
    };
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
///
/// Descends through plain `TransformComponent` wrappers (common pattern:
/// `T.position(…){ Text { … } }`) so a box can measure text wrapped in a
/// positioning inner T. Halts at nested *styled* layout items — those are
/// their own boxes and shouldn't bleed text up here.
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
            // Return the *authored* wrap_at — callers use this as a hard cap
            // against the current container width. `t.wrap_at` reflects a prior
            // pass's already-narrowed value and would prevent re-widening when
            // the container grows.
            return Some((
                t.text.clone(),
                t.authored_wrap_at,
                t.word_wrap,
                t.word_wrap_tokens.clone(),
            ));
        }

        if node != root {
            if world.get_component_by_id_as::<LayoutComponent>(node).is_some() {
                return None;
            }
            let is_boundary = world.children_of(node).iter().any(|&ch| {
                world.get_component_by_id_as::<StyleComponent>(ch).is_some()
                    || world.get_component_by_id_as::<HtmlElementComponent>(ch).is_some()
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
    // Glyph quads are centered at column positions, so the rightmost glyph
    // spans [col-0.5, col+0.5]. Reserve half a glyph on the right edge so the
    // last glyph's right half fits inside the content box (and inside padding).
    let wrap_at = if content_width_gu > CHAR_WIDTH_GU {
        let container_cols = container_cols_for_width(content_width_gu);
        if existing_wrap_at == 0 { container_cols } else { container_cols.min(existing_wrap_at) }
    } else if existing_wrap_at == 0 {
        // No container width and no author cap — measure unwrapped.
        usize::MAX
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
    let container_cols = container_cols_for_width(content_width_gu);

    // Style overrides on the styled TC propagate onto the descendant TextComponent.
    // No cascade today — only this TC's own StyleComponent is consulted.
    let (style_word_wrap, style_tokens) = read_text_wrap_style(world, tc_id);

    let (cur_wrap_at, authored_wrap_at, cur_word_wrap, cur_tokens, cur_text) =
        match world.get_component_by_id_as::<TextComponent>(text_id) {
            Some(tc) => (tc.wrap_at, tc.authored_wrap_at, tc.word_wrap, tc.word_wrap_tokens.clone(), tc.text.clone()),
            None => return,
        };

    // Cap against the *authored* wrap_at, not the current value (the current
    // value may have been narrowed by a prior layout pass at a smaller
    // container width — capping there would prevent re-widening).
    // An authored cap of `0` means "no author limit" — fill the container.
    let new_wrap_at = if authored_wrap_at == 0 {
        container_cols
    } else {
        container_cols.min(authored_wrap_at)
    };
    let new_word_wrap = match style_word_wrap {
        Some(WordWrapMode::Normal)    => false,
        Some(WordWrapMode::BreakWord) => true,
        None                          => cur_word_wrap,
    };
    let new_tokens = style_tokens.unwrap_or_else(|| cur_tokens.clone());

    if new_wrap_at == cur_wrap_at
        && new_word_wrap == cur_word_wrap
        && new_tokens == cur_tokens
    {
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
        if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
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
        if node != root && world.get_component_by_id_as::<TransformComponent>(node).is_some() {
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
) -> Option<f32> {
    let is_inline_block = matches!(display, Some(Display::InlineBlock | Display::Inline));
    if find_text_in_local_content_subtree(world, tc_id).is_some() {
        // CSS-aligned: inline-block shrinks to fit its content; block fills
        // the available inline budget so text wraps inside it.
        if is_inline_block {
            return Some(text_intrinsic_width(world, tc_id, avail_content_w_gu));
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
fn text_intrinsic_width(world: &World, tc_id: ComponentId, avail_content_w_gu: f32) -> f32 {
    let Some((text, tc_wrap_at, word_wrap, tokens)) =
        find_text_in_local_content_subtree(world, tc_id)
    else {
        return 0.0;
    };
    let avail_cols = if avail_content_w_gu > CHAR_WIDTH_GU {
        container_cols_for_width(avail_content_w_gu)
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
    let (max_col, _line_count) = TextSystem::measure(&text, wrap_at, word_wrap, &tokens);
    max_col as f32 * CHAR_WIDTH_GU
}

fn intrinsic_block_height(world: &World, tc_id: ComponentId, content_width_gu: f32) -> f32 {
    if find_text_in_local_content_subtree(world, tc_id).is_some() {
        return text_intrinsic_height(world, tc_id, content_width_gu);
    }

    if let Some(aabb) = find_renderable_local_bounds(world, tc_id) {
        return aabb.height();
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

use crate::engine::ecs::component::style::{BoxSizing, EdgeInsets};

type StyleTuple = (EdgeInsets, EdgeInsets, SizeDimension, SizeDimension, Option<Display>, f32, BoxSizing);

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
    use super::{measure_item, measure_items};
    use crate::engine::ecs::component::{ColorComponent, LayoutComponent, StyleComponent, TextComponent, TransformComponent};
    use crate::engine::ecs::component::style::{Display, SizeDimension};
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

        let measured = measure_item(&world, tc, 40.0);
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

        let measured = measure_item(&world, tc, 40.0);
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

        let measured = measure_item(&world, tc, 40.0);
        assert_eq!(measured.content_width_gu, 12.0);
    }

    #[test]
    fn text_wrap_relaxes_when_container_grows_back() {
        use crate::engine::ecs::SignalEmitter;
        use crate::engine::ecs::ComponentId;
        use crate::engine::ecs::rx::{EventSignal, IntentSignal};
        use super::apply_text_wrap_for_item;

        struct NullEmit;
        impl SignalEmitter for NullEmit {
            fn push_event(&mut self, _: ComponentId, _: EventSignal) {}
            fn push_intent(&mut self, _: ComponentId, _: IntentSignal) {}
        }

        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let text = world.add_component_boxed_named(
            "txt",
            Box::new(TextComponent::with_word_wrap("the quick brown fox jumps over the lazy dog", 60)),
        );
        let _ = world.add_child(tc, text);

        let mut emit = NullEmit;

        // 1st pass: narrow container forces wrap_at down.
        apply_text_wrap_for_item(&mut world, &mut emit, tc, 10.0);
        let narrow = world.get_component_by_id_as::<TextComponent>(text).unwrap().wrap_at;
        assert!(narrow < 60, "narrow container should reduce wrap_at, got {}", narrow);

        // 2nd pass: container grows back; wrap_at must widen toward the authored cap.
        apply_text_wrap_for_item(&mut world, &mut emit, tc, 80.0);
        let wide = world.get_component_by_id_as::<TextComponent>(text).unwrap().wrap_at;
        assert!(wide > narrow, "wide container should re-widen wrap_at, got {} (was {})", wide, narrow);
        assert!(wide <= 60, "wrap_at must never exceed authored cap (60), got {}", wide);
    }

    #[test]
    fn content_box_explicit_width_with_padding_grows_outer_box() {
        use crate::engine::ecs::component::style::{BoxSizing, EdgeInsets};
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named("style", Box::new({
            let mut s = StyleComponent::new();
            s.display = Some(Display::Block);
            s.box_sizing = BoxSizing::ContentBox;
            s.width = SizeDimension::GlyphUnits(20.0);
            s.padding = EdgeInsets::all(2.0);
            s
        }));
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0);
        assert_eq!(measured.content_width_gu, 20.0, "content stays at width(20) under content-box");
        assert_eq!(measured.box_width_gu, 24.0, "outer box = content + 2*padding");
    }

    #[test]
    fn border_box_explicit_width_with_padding_keeps_outer_box_width() {
        use crate::engine::ecs::component::style::EdgeInsets;
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named("style", Box::new({
            let mut s = StyleComponent::new();
            s.display = Some(Display::Block);
            s.width = SizeDimension::GlyphUnits(20.0);
            s.padding = EdgeInsets::all(2.0);
            s
        }));
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0);
        assert_eq!(measured.box_width_gu, 20.0, "outer box stays at width(20)");
        assert_eq!(measured.content_width_gu, 16.0, "content shrinks for padding");
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
        assert!((items[0].margin_box_width_gu + items[1].margin_box_width_gu - 80.0).abs() < 1e-4,
            "got {} + {} = {}, expected 80",
            items[0].margin_box_width_gu,
            items[1].margin_box_width_gu,
            items[0].margin_box_width_gu + items[1].margin_box_width_gu);
        assert!((items[0].margin_box_width_gu - 20.0).abs() < 1e-4);
        assert!((items[1].margin_box_width_gu - 60.0).abs() < 1e-4);
    }

    #[test]
    fn width_percent_resolves_against_available_content_width() {
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named("style", Box::new({
            let mut s = StyleComponent::new();
            s.display = Some(Display::Block);
            s.width = SizeDimension::Percent(50.0);
            s
        }));
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0);
        assert_eq!(measured.content_width_gu, 20.0);
    }

    #[test]
    fn padding_percent_resolves_against_inline_axis_width() {
        use crate::engine::ecs::component::style::EdgeInsets;
        let mut world = World::default();
        let tc = world.add_component_boxed_named("tc", Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named("style", Box::new({
            let mut s = StyleComponent::new();
            s.display = Some(Display::Block);
            s.width = SizeDimension::GlyphUnits(20.0);
            s.padding = EdgeInsets::all_dim(SizeDimension::Percent(10.0));
            s
        }));
        let _ = world.add_child(tc, style);

        let measured = measure_item(&world, tc, 40.0);
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
        let style = world.add_component_boxed_named("style", Box::new({
            let mut s = StyleComponent::new();
            s.display = Some(Display::Block);
            s.width = SizeDimension::GlyphUnits(20.0);
            s
        }));
        let _ = world.add_child(tc, style);
        let measured = measure_item(&world, tc, 40.0);
        assert_eq!(measured.content_width_gu, 20.0);
    }
}
