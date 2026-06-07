use super::box_model_viz::sync_box_model_viz;
use super::measure::{
    MeasuredItem, apply_text_color_for_item, apply_text_font_size_for_item,
    apply_text_wrap_for_item, measure_container_items, measure_items,
};
use crate::engine::ecs::ComponentId;
/// Block formatting context layout — Pass 2.
///
/// Children stack top-to-bottom with a vertical cursor.
/// Each item contributes `margin_top + box_height + margin_bottom` to the cursor.
/// The TC is positioned at the content-box origin:
///   `x = (margin_left + padding_left) * unit_scale`
///   `y = -(margin_top + padding_top) * unit_scale`  (relative to cursor before margin)
///
/// No horizontal cursor — block items start at their own left margin + padding
/// and stretch to fill available width.
///
/// Background quads (`Style { background_color }`) are spawned as `__bg` children of
/// each item TC and sized to cover the full padding box. The item TC is left at
/// scale 1.0; `__bg`'s translation and scale are multiplied by `unit_scale` so that
/// glyph-unit measurements convert to world units correctly regardless of whether
/// the layout root is being scaled by an outer transform (`unit_scale = 1.0`) or
/// by `unit_scale` itself (e.g. inspector panels with `unit_scale = TEXT_SCALE`).
use crate::engine::ecs::World;
use crate::engine::ecs::component::style::VerticalAlign;
use crate::engine::ecs::component::style::{Display, SizeDimension, TextAlign};
use crate::engine::ecs::component::{
    ColorComponent, InspectLayoutComponent, LayoutBoundsComponent, OpacityComponent, Overflow,
    RaycastableComponent, RaycastableShapeComponent, RaycastableShapeType, RenderableComponent,
    RouterComponent, ScrollingComponent, SerializeComponent, StencilClipComponent, StyleComponent,
    TextComponent, TransformComponent,
};
use crate::engine::ecs::system::ScrollingSystem;
use crate::engine::ecs::system::text_system::TextSystem;
use crate::engine::ecs::{IntentValue, SignalEmitter};

const OWNED_CLIPPED_CONTENT_LABEL: &str = "__clip_content";
const OWNED_LAYOUT_STENCIL_CLIP_LABEL: &str = "__layout_stencil_clip";
const OWNED_LAYOUT_OVERFLOW_ROUTER_LABEL: &str = "__layout_overflow_router";
const OWNED_SCROLL_WRAPPER_LABEL: &str = "__scroll";
const OWNED_SCROLL_ROUTER_LABEL: &str = "__scroll_router";
const OWNED_SCROLL_TRACK_LABEL: &str = "__scroll_track";
const OWNED_SCROLL_DRAG_RAYCASTABLE_LABEL: &str = "__scroll_drag_raycastable";
const OWNED_SCROLL_DRAG_SHAPE_LABEL: &str = "__scroll_drag_shape";
const OWNED_BG_RAYCASTABLE_LABEL: &str = "__bg_raycastable";
const OWNED_BG_RAYCASTABLE_SHAPE_LABEL: &str = "__bg_raycastable_shape";
const OWNED_LAYOUT_BOUNDS_LABEL: &str = "__layout_bounds";

/// Run a block formatting context layout pass for `layout_id`.
///
/// Calls `measure_items` (Pass 1) then walks the results with a vertical cursor,
/// emits `UpdateTransform` for each TC child, and manages background quads for
/// items with `Style { background_color }`.
///
/// Returns `(total_width_gu, total_height_gu)` — the total extent of the
/// top-level items in glyph units.
pub fn layout(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    layout_id: ComponentId,
) -> (f32, f32) {
    let (items, _avail_w, _avail_h, unit_scale) = measure_items(world, layout_id);
    let viz = layout_root_has_inspect(world, layout_id);
    let axis_scales = super::measure::layout_root_axis_scales(world, layout_id);

    layout_items(world, emit, &items, unit_scale, axis_scales, 0, 0, viz);

    let total_height_gu: f32 = items.iter().map(|i| i.margin_box_height_gu).sum();
    (_avail_w, total_height_gu)
}

/// Public-to-the-layout-module entry so `inline::layout_items` can recurse
/// back into block flow when an inline-block item's children are block-level.
pub(crate) fn layout_items_for(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    items: &[MeasuredItem],
    unit_scale: f32,
    depth: i32,
    parent_depth: i32,
    viz: bool,
) {
    layout_items(
        world,
        emit,
        items,
        unit_scale,
        (1.0, 1.0),
        depth,
        parent_depth,
        viz,
    );
}

/// `depth` is the *layer* depth from the LayoutRoot (0 at root). Sibling items
/// at one level share the same resolved Z; the depth only advances when
/// recursing into an item that *owns a layer* (has its own `__bg` quad or an
/// overflow clip) — items without a bg are structural and don't need their
/// children to sit on a new Z plane. `viz` propagates the LayoutRoot's
/// [`InspectLayoutComponent`] presence so each item's box-model viz can be
/// toggled per layout tree.
fn layout_items(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    items: &[MeasuredItem],
    unit_scale: f32,
    axis_scales: (f32, f32),
    depth: i32,
    parent_depth: i32,
    viz: bool,
) {
    let mut cursor_gu = 0.0_f32;
    let resolved_z = (depth - parent_depth) as f32 * super::LAYER_DISTANCE;

    for item in items {
        cursor_gu += item.margin_top_gu;

        let content_origin_y_gu = cursor_gu + item.padding_top_gu;
        let content_origin_x_gu = item.margin_left_gu + item.padding_left_gu;

        // LayoutSystem owns X/Y/Z on styled item TCs. Z is overwritten with the
        // layer-resolved value each pass; composing with the prior TC translation
        // would drift on re-layout because we'd read back our own write as
        // "authored". Per-item Z bias is left to `Style.z_index` (currently inert
        // — see `docs/spec/layout-stacking-z-index.md`).
        let tc_scale = world
            .get_component_by_id_as::<TransformComponent>(item.tc_id)
            .map(|tc| tc.transform.scale)
            .unwrap_or([1.0, 1.0, 1.0]);

        let composed_z = resolved_z;
        let translation = [
            content_origin_x_gu * unit_scale,
            -(content_origin_y_gu * unit_scale),
            composed_z,
        ];

        emit.push_intent_now(
            item.tc_id,
            IntentValue::UpdateTransform {
                component_ids: vec![item.tc_id],
                translation,
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: tc_scale,
            },
        );

        sync_layout_bounds(world, emit, item, unit_scale);

        // Push the container-derived wrap_at into any descendant TextComponent
        // and rebuild glyphs so the rendered text matches the measured width.
        apply_text_font_size_for_item(world, emit, item.tc_id, unit_scale);
        apply_text_wrap_for_item(world, emit, item.tc_id, item.content_width_gu, unit_scale);
        apply_text_color_for_item(world, emit, item.tc_id);

        // ── Background quad / overflow helper topology ───────────────────
        sync_bg_quad(
            world,
            emit,
            item.tc_id,
            item.padding_left_gu,
            item.padding_top_gu,
            item.box_width_gu,
            item.box_height_gu,
            unit_scale,
        );
        sync_auto_text_lift(world, emit, item.tc_id);
        sync_box_model_viz(world, emit, item, unit_scale, viz);
        apply_text_align(
            world,
            emit,
            item.tc_id,
            item.content_width_gu,
            item.content_height_gu,
            unit_scale,
        );
        let content_root = sync_overflow_topology(world, emit, item.tc_id, item.content_height_gu);

        let nested_items = measure_container_items(
            world,
            content_root,
            item.content_width_gu,
            Some(item.content_height_gu),
            unit_scale,
        );
        if let Some(scroll_id) = immediate_owned_scroll_wrapper(world, item.tc_id) {
            sync_scrolling_metrics(
                world,
                emit,
                scroll_id,
                item.content_height_gu,
                &nested_items,
            );
        }
        if !nested_items.is_empty() {
            // Switch formatting context per subtree: when every nested item
            // is inline-block, run them through the inline cursor + wrap
            // path; otherwise stay in block flow. Mirrors the dispatch in
            // `LayoutSystem::run_layout` but applied at every level so
            // mixed trees under a single LayoutRoot work.
            let all_inline_block = nested_items
                .iter()
                .all(|it| matches!(it.display, Some(Display::InlineBlock | Display::Inline)));
            // Children only sit on a new Z layer when *this* item owns one
            // (has a bg or an overflow clip). Structural wrappers without
            // their own bg keep their children on the parent's layer, so
            // deeply-nested layout trees don't accumulate Z by accident.
            let child_depth = if item_owns_layer(world, item.tc_id) {
                depth + 1
            } else {
                depth
            };
            if all_inline_block {
                super::inline::layout_items(
                    world,
                    emit,
                    &nested_items,
                    item.content_width_gu,
                    unit_scale,
                    axis_scales,
                    child_depth,
                    depth,
                    viz,
                );
            } else {
                layout_items(
                    world,
                    emit,
                    &nested_items,
                    unit_scale,
                    axis_scales,
                    child_depth,
                    depth,
                    viz,
                );
            }
        }

        cursor_gu += item.box_height_gu + item.margin_bottom_gu;
    }
}

/// Auto-lift the immediate non-styled TC children of a styled item so their
/// content (typically a `Text { … }` wrapper) sits clearly ahead of the item's
/// own `__bg` quad without authors having to hand-author `T.position(_,_,Z)`.
///
/// Only fires when the child's authored local Z is exactly `0.0` — any
/// non-zero value is treated as an explicit author override and preserved.
/// Layout-owned helpers (labels starting with `__`) and TC children that are
/// themselves layout items are skipped: the former have their own placement
/// rules (e.g. `__bg`), the latter get full layer treatment from the recursive
/// `layout_items` pass.
pub(crate) fn sync_auto_text_lift(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
) {
    let candidates: Vec<ComponentId> = world
        .children_of(tc_id)
        .iter()
        .copied()
        .filter(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
                && !world
                    .component_label(child)
                    .map(|label| label.starts_with("__"))
                    .unwrap_or(false)
                && !super::measure::is_layout_item(world, child)
        })
        .collect();

    for child in candidates {
        let Some(tc) = world.get_component_by_id_as::<TransformComponent>(child) else {
            continue;
        };
        if tc.transform.translation[2] != 0.0 {
            continue;
        }
        let translation = [
            tc.transform.translation[0],
            tc.transform.translation[1],
            super::AUTO_TEXT_LIFT_Z,
        ];
        let scale = tc.transform.scale;
        let rotation_quat_xyzw = tc.transform.rotation;
        emit.push_intent_now(
            child,
            IntentValue::UpdateTransform {
                component_ids: vec![child],
                translation,
                rotation_quat_xyzw,
                scale,
            },
        );
    }
}

/// Does this styled item carry rendering content that justifies a new Z
/// layer for its children? `true` when it has a `__bg` quad (i.e.
/// `Style.background_color` is `Some`) or any clipping overflow mode.
/// Structural wrappers without their own bg return `false` and so don't
/// push their children onto a deeper layer.
pub(crate) fn item_owns_layer(world: &World, tc_id: ComponentId) -> bool {
    world.children_of(tc_id).iter().any(|&ch| {
        world
            .get_component_by_id_as::<StyleComponent>(ch)
            .map(|s| s.background_color.is_some() || !matches!(s.overflow, Overflow::Visible))
            .unwrap_or(false)
    })
}

pub(crate) fn layout_root_has_inspect(world: &World, layout_id: ComponentId) -> bool {
    use crate::engine::ecs::component::LayoutComponent;
    let flag = world
        .get_component_by_id_as::<LayoutComponent>(layout_id)
        .map(|l| l.inspect)
        .unwrap_or(false);
    flag || world.children_of(layout_id).iter().any(|&ch| {
        world
            .get_component_by_id_as::<InspectLayoutComponent>(ch)
            .is_some()
    })
}

fn style_overflow(world: &World, tc_id: ComponentId) -> Overflow {
    world
        .children_of(tc_id)
        .iter()
        .find_map(|&child| {
            world
                .get_component_by_id_as::<StyleComponent>(child)
                .map(|style| style.overflow)
        })
        .unwrap_or(Overflow::Visible)
}

fn immediate_owned_layout_router(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_LAYOUT_OVERFLOW_ROUTER_LABEL)
            && world
                .get_component_by_id_as::<RouterComponent>(child)
                .is_some()
    })
}

fn immediate_owned_clipped_content(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_CLIPPED_CONTENT_LABEL)
            && world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
    })
}

fn immediate_owned_scroll_wrapper(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_SCROLL_WRAPPER_LABEL)
            && world
                .get_component_by_id_as::<ScrollingComponent>(child)
                .is_some()
    })
}

fn immediate_owned_scroll_track(world: &World, scroll_id: ComponentId) -> Option<ComponentId> {
    world.children_of(scroll_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_SCROLL_TRACK_LABEL)
            && world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
    })
}

fn immediate_owned_scroll_router(world: &World, scroll_id: ComponentId) -> Option<ComponentId> {
    world.children_of(scroll_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_SCROLL_ROUTER_LABEL)
            && world
                .get_component_by_id_as::<RouterComponent>(child)
                .is_some()
    })
}

fn ensure_scroll_track(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    scroll_id: ComponentId,
) -> ComponentId {
    let track_id = if let Some(track_id) = immediate_owned_scroll_track(world, scroll_id) {
        track_id
    } else {
        let track_id = world.add_component_boxed_named(
            OWNED_SCROLL_TRACK_LABEL,
            Box::new(TransformComponent::new()),
        );
        let _ = world.add_child(scroll_id, track_id);
        world.init_component_tree(track_id, emit);
        track_id
    };

    if immediate_owned_scroll_router(world, scroll_id).is_none() {
        let router_id = world.add_component_boxed_named(
            OWNED_SCROLL_ROUTER_LABEL,
            Box::new(RouterComponent::new().with_target_name(OWNED_SCROLL_TRACK_LABEL)),
        );
        let _ = world.add_child(scroll_id, router_id);
        world.init_component_tree(router_id, emit);
    }

    let base_pos = world
        .get_component_by_id_as::<TransformComponent>(track_id)
        .map(|tc| tc.transform.translation)
        .unwrap_or([0.0, 0.0, 0.0]);

    if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_id) {
        if sc.track != Some(track_id) {
            sc.set_track(track_id, base_pos);
        }
    }

    track_id
}

fn ensure_overflow_router(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    owner: ComponentId,
    target_name: &str,
) -> ComponentId {
    if let Some(router_id) = immediate_owned_layout_router(world, owner) {
        if let Some(router) = world.get_component_by_id_as_mut::<RouterComponent>(router_id) {
            router.target_name = Some(target_name.to_string());
        }
        return router_id;
    }

    let router_id = world.add_component_boxed_named(
        OWNED_LAYOUT_OVERFLOW_ROUTER_LABEL,
        Box::new(RouterComponent::new().with_target_name(target_name)),
    );
    let _ = world.add_child(owner, router_id);
    world.init_component_tree(router_id, emit);
    router_id
}

fn ensure_clipped_content_root(world: &mut World, owner: ComponentId) -> ComponentId {
    if let Some(content_id) = immediate_owned_clipped_content(world, owner) {
        return content_id;
    }

    let content_id = world.add_component_boxed_named(
        OWNED_CLIPPED_CONTENT_LABEL,
        Box::new(TransformComponent::new()),
    );
    let _ = world.add_child(owner, content_id);
    content_id
}

fn ensure_scroll_wrapper(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    owner: ComponentId,
    viewport_height: f32,
) -> ComponentId {
    if let Some(scroll_id) = immediate_owned_scroll_wrapper(world, owner) {
        if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_id) {
            sc.viewport_height = viewport_height.max(0.0);
            sc.content_height = sc.content_height.max(viewport_height.max(0.0));
            let _ = sc.clamp_to_content();
        }
        let _ = ensure_scroll_track(world, emit, scroll_id);
        return scroll_id;
    }

    let scroll_id = world.add_component_boxed_named(
        OWNED_SCROLL_WRAPPER_LABEL,
        Box::new(ScrollingComponent::new(
            viewport_height.max(0.0),
            viewport_height.max(0.0),
        )),
    );
    let _ = world.add_child(owner, scroll_id);
    world.init_component_tree(scroll_id, emit);
    let _ = ensure_scroll_track(world, emit, scroll_id);
    scroll_id
}

fn authored_overflow_children(world: &World, owner: ComponentId) -> Vec<ComponentId> {
    world
        .children_of(owner)
        .iter()
        .copied()
        .filter(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
                && !world
                    .component_label(child)
                    .map(|label| label.starts_with("__"))
                    .unwrap_or(false)
        })
        .collect()
}

fn relocate_authored_children(world: &mut World, owner: ComponentId, target: ComponentId) {
    for child in authored_overflow_children(world, owner) {
        let _ = world.add_child(target, child);
    }
}

fn scroll_content_root(world: &World, scroll_id: ComponentId) -> ComponentId {
    immediate_owned_scroll_track(world, scroll_id)
        .or_else(|| {
            world
                .get_component_by_id_as::<ScrollingComponent>(scroll_id)
                .and_then(|sc| sc.track)
        })
        .unwrap_or(scroll_id)
}

fn sync_scrolling_metrics(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    scroll_id: ComponentId,
    viewport_height: f32,
    nested_items: &[MeasuredItem],
) {
    let content_height = nested_items
        .iter()
        .map(|item| item.margin_box_height_gu)
        .sum::<f32>();

    if let Some(sc) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_id) {
        sc.viewport_height = viewport_height.max(0.0);
        let _ = sc.clamp_to_content();
    }
    ScrollingSystem::set_content_height(world, emit, scroll_id, content_height);
    ScrollingSystem::sync_component(world, emit, scroll_id);
}

pub(crate) fn sync_overflow_topology(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
    viewport_height: f32,
) -> ComponentId {
    match style_overflow(world, tc_id) {
        Overflow::Hidden => {
            let content_root = ensure_clipped_content_root(world, tc_id);
            ensure_overflow_router(world, emit, tc_id, OWNED_CLIPPED_CONTENT_LABEL);
            relocate_authored_children(world, tc_id, content_root);
            content_root
        }
        Overflow::Scroll => {
            let scroll_id = ensure_scroll_wrapper(world, emit, tc_id, viewport_height);
            ensure_overflow_router(world, emit, tc_id, OWNED_SCROLL_WRAPPER_LABEL);
            let content_root = scroll_content_root(world, scroll_id);
            relocate_authored_children(world, tc_id, content_root);
            content_root
        }
        _ => tc_id,
    }
}

/// Create, update, or remove the `__bg` child TC for a layout item.
///
/// The background quad covers the full padding box (content + padding on all sides).
/// Glyph-unit measurements are converted to world units via `unit_scale`, since the
/// item TC is at scale 1.0 in the parent layout's local space.
pub(crate) fn sync_bg_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
    padding_left_gu: f32,
    padding_top_gu: f32,
    box_width_gu: f32,
    box_height_gu: f32,
    unit_scale: f32,
) {
    // Collect children to avoid holding a borrow on world during mutation.
    let children: Vec<ComponentId> = world.children_of(tc_id).to_vec();

    let bg_style = children.iter().find_map(|&ch| {
        world
            .get_component_by_id_as::<StyleComponent>(ch)
            .map(|s| (s.background_color, s.background_z, s.overflow))
    });

    let existing_bg = children
        .iter()
        .find(|&&ch| world.component_label(ch) == Some("__bg"))
        .copied();

    // The bg quad is a child of the item TC; its translation is in the item's
    // local frame, which already sits at the item's layer-resolved Z. So the
    // default bg only needs the half-step offset behind its parent's content
    // origin. `Style.background_z` overrides this absolute (still in local Z).
    let default_bg_z = -0.5 * super::LAYER_DISTANCE;

    let (needs_clip, needs_scroll_drag_surface, bg_spec) = match bg_style {
        Some((rgba, bg_z_override, overflow)) => (
            matches!(overflow, Overflow::Hidden | Overflow::Scroll),
            matches!(overflow, Overflow::Scroll),
            Some((rgba, bg_z_override.unwrap_or(default_bg_z))),
        ),
        None => (false, false, None),
    };

    if let Some((rgba, bg_z)) = bg_spec {
        let needs_bg = rgba.is_some() || needs_clip;
        if needs_bg {
            let bg_id = match existing_bg {
                Some(id) => id,
                None => spawn_bg_quad(world, emit, tc_id, rgba.unwrap_or([0.0, 0.0, 0.0, 0.0])),
            };

            if let Some(color_rgba) = rgba {
                if let Some(color_id) = world.children_of(bg_id).iter().copied().find(|&child| {
                    world
                        .get_component_by_id_as::<ColorComponent>(child)
                        .is_some()
                }) {
                    emit.push_intent_now(
                        color_id,
                        IntentValue::SetColor {
                            component_ids: vec![color_id],
                            rgba: color_rgba,
                        },
                    );
                }
            }

            emit.push_intent_now(
                bg_id,
                IntentValue::UpdateTransform {
                    component_ids: vec![bg_id],
                    translation: [
                        (box_width_gu / 2.0 - padding_left_gu) * unit_scale,
                        (padding_top_gu - box_height_gu / 2.0) * unit_scale,
                        bg_z,
                    ],
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [box_width_gu * unit_scale, box_height_gu * unit_scale, 1.0],
                },
            );

            sync_stencil_clip(world, emit, tc_id, needs_clip);
            sync_scroll_drag_surface(world, emit, bg_id, needs_scroll_drag_surface);
            let author_rc = find_author_raycastable(world, tc_id);
            sync_bg_author_raycastable(world, emit, bg_id, author_rc);
            return;
        }
    }

    sync_stencil_clip(world, emit, tc_id, false);
    if let Some(bg_id) = existing_bg {
        sync_scroll_drag_surface(world, emit, bg_id, false);
        sync_bg_author_raycastable(world, emit, bg_id, None);
        emit.push_intent_now(
            bg_id,
            IntentValue::RemoveSubtree {
                component_ids: vec![bg_id],
            },
        );
    }
}

fn immediate_owned_layout_bounds(world: &World, tc_id: ComponentId) -> Option<ComponentId> {
    world.children_of(tc_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_LAYOUT_BOUNDS_LABEL)
            && world
                .get_component_by_id_as::<LayoutBoundsComponent>(child)
                .is_some()
    })
}

pub(crate) fn sync_layout_bounds(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    item: &MeasuredItem,
    unit_scale: f32,
) {
    let content_local = crate::engine::graphics::bounds::Aabb {
        min: [0.0, -item.content_height_gu * unit_scale, 0.0],
        max: [item.content_width_gu * unit_scale, 0.0, 0.0],
    };
    let padding_local = crate::engine::graphics::bounds::Aabb {
        min: [
            -item.padding_left_gu * unit_scale,
            (item.padding_top_gu - item.box_height_gu) * unit_scale,
            0.0,
        ],
        max: [
            (item.box_width_gu - item.padding_left_gu) * unit_scale,
            item.padding_top_gu * unit_scale,
            0.0,
        ],
    };

    if let Some(existing) = immediate_owned_layout_bounds(world, item.tc_id) {
        if let Some(bounds) = world.get_component_by_id_as_mut::<LayoutBoundsComponent>(existing) {
            bounds.content_local = content_local;
            bounds.padding_local = padding_local;
        }
        return;
    }

    let bounds_id = world.add_component_boxed_named(
        OWNED_LAYOUT_BOUNDS_LABEL,
        Box::new(LayoutBoundsComponent::new(content_local, padding_local)),
    );
    let _ = world.add_child(item.tc_id, bounds_id);
    world.init_component_tree(bounds_id, emit);
}

fn immediate_owned_layout_stencil_clip(
    world: &World,
    scope_root: ComponentId,
) -> Option<ComponentId> {
    world
        .children_of(scope_root)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) == Some(OWNED_LAYOUT_STENCIL_CLIP_LABEL)
                && world
                    .get_component_by_id_as::<StencilClipComponent>(child)
                    .is_some()
        })
}

/// Attach or detach the layout-owned `StencilClipComponent` as a sibling of `__bg`.
fn sync_stencil_clip(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    scope_root: ComponentId,
    needs_clip: bool,
) {
    if needs_clip {
        if immediate_owned_layout_stencil_clip(world, scope_root).is_none() {
            let clip_id = world.add_component_boxed_named(
                OWNED_LAYOUT_STENCIL_CLIP_LABEL,
                Box::new(StencilClipComponent::new()),
            );
            let _ = world.add_child(scope_root, clip_id);
            world.init_component_tree(clip_id, emit);
        }
    } else if let Some(clip_id) = immediate_owned_layout_stencil_clip(world, scope_root) {
        emit.push_intent_now(
            clip_id,
            IntentValue::RemoveSubtree {
                component_ids: vec![clip_id],
            },
        );
    }
}

fn subtree_first_renderable(world: &World, root: ComponentId) -> Option<ComponentId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        for &child in world.children_of(node).iter().rev() {
            stack.push(child);
        }
    }
    None
}

/// Position the inner text-bearing T inside a styled box per `Style.text_align`.
///
/// Layout positions the *styled* T at the content box's top-left corner; without
/// extra help, any Text inside renders from that origin and sits at the top-left
/// of the content area. When the author sets `text_align` or `vertical_align`,
/// locate the first direct-child T whose subtree contains a `TextComponent`,
/// measure the text, and emit `UpdateTransform` so the glyph block sits aligned
/// within the content box. `VerticalAlign::Auto` preserves the legacy behavior:
/// if `text_align` is set, text is vertically centered; otherwise the author's
/// translation is preserved.
pub(crate) fn apply_text_align(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
    content_w_gu: f32,
    content_h_gu: f32,
    unit_scale: f32,
) {
    let style = world.children_of(tc_id).iter().find_map(|&ch| {
        world.get_component_by_id_as::<StyleComponent>(ch).map(|s| {
            (
                s.text_align,
                s.vertical_align,
                s.font_size,
                s.word_wrap,
                s.word_wrap_tokens.clone(),
            )
        })
    });
    let Some((align, vertical_align, style_font_size, _style_wrap, _style_tokens)) = style else {
        return;
    };

    // No alignment intent → leave the author's inner-T transform alone. This
    // preserves the "decorative descendant text" pattern (e.g. a title-bar T
    // with an absolutely positioned label) where the author has placed the
    // glyph block deliberately and the layout system has no business shoving
    // it half-a-glyph from the content-box origin. A styled item with an
    // explicit `font_size` opts into the inset (signals "this box hosts text
    // and wants it sized to its content area").
    if align == TextAlign::Auto
        && vertical_align == VerticalAlign::Auto
        && matches!(style_font_size, SizeDimension::Auto)
    {
        return;
    }

    let Some(inner_tc) = find_alignable_direct_child(world, tc_id) else {
        return;
    };

    // Measure text directly off the inner T's TextComponent so we get the
    // post-wrap shape (callers of layout run `apply_text_wrap_for_item` later,
    // but glyphs build with the authored wrap_at and we want the natural-
    // wrap width here — i.e. no wrap — to drive alignment math).
    let (text_w_wu, text_h_wu) = match find_text_descriptor(world, inner_tc) {
        Some((text, word_wrap, tokens, font_size_wu)) => {
            TextSystem::measure(&text, 0, word_wrap, &tokens, font_size_wu)
        }
        None => (0.0, 0.0), // Non-text children are treated as 0-sized anchors at their origin
    };

    let content_w_wu = content_w_gu * unit_scale;
    let content_h_wu = content_h_gu * unit_scale;

    let (translation, scale) = world
        .get_component_by_id_as::<TransformComponent>(inner_tc)
        .map(|tc| (tc.transform.translation, tc.transform.scale))
        .unwrap_or(([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]));

    let x_translation = match align {
        TextAlign::Left => 0.0,
        TextAlign::Right => (content_w_wu - text_w_wu).max(0.0),
        TextAlign::Center => ((content_w_wu - text_w_wu) * 0.5).max(0.0),
        TextAlign::Auto => 0.0,
    };
    let y_translation = match vertical_align {
        VerticalAlign::Top => 0.0,
        VerticalAlign::Middle => -((content_h_wu - text_h_wu) * 0.5).max(0.0),
        VerticalAlign::Bottom => -(content_h_wu - text_h_wu).max(0.0),
        VerticalAlign::Auto if align != TextAlign::Auto => {
            -((content_h_wu - text_h_wu) * 0.5).max(0.0)
        }
        VerticalAlign::Auto => 0.0,
    };

    emit.push_intent_now(
        inner_tc,
        IntentValue::UpdateTransform {
            component_ids: vec![inner_tc],
            translation: [x_translation, y_translation, translation[2]],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale,
        },
    );
}

fn find_alignable_direct_child(world: &World, tc_id: ComponentId) -> Option<ComponentId> {
    for &child in world.children_of(tc_id) {
        if world
            .component_label(child)
            .map(|l| l.starts_with("__"))
            .unwrap_or(false)
        {
            continue;
        }
        if world
            .get_component_by_id_as::<TransformComponent>(child)
            .is_some()
        {
            return Some(child);
        }
    }
    None
}

fn subtree_has_text(world: &World, root: ComponentId) -> bool {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<TextComponent>(node)
            .is_some()
        {
            return true;
        }
        for &child in world.children_of(node) {
            // Don't descend into another styled layout item.
            if world
                .get_component_by_id_as::<StyleComponent>(child)
                .is_some()
                && child != root
            {
                continue;
            }
            stack.push(child);
        }
    }
    false
}

fn find_text_descriptor(
    world: &World,
    root: ComponentId,
) -> Option<(String, bool, Vec<String>, f32)> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(t) = world.get_component_by_id_as::<TextComponent>(node) {
            return Some((
                t.text.clone(),
                t.word_wrap,
                t.word_wrap_tokens.clone(),
                t.font_size,
            ));
        }
        for &child in world.children_of(node) {
            stack.push(child);
        }
    }
    None
}

/// Find an author-written `RaycastableComponent` that is an immediate child of `tc_id`,
/// ignoring layout-owned `__*` subtrees. This is the explicit author signal that the
/// styled box should be hittable; the layout system grafts a copy onto the `__bg`
/// renderable so raycast pairing finds it.
fn find_author_raycastable(world: &World, tc_id: ComponentId) -> Option<RaycastableComponent> {
    world.children_of(tc_id).iter().copied().find_map(|child| {
        if world
            .component_label(child)
            .map(|l| l.starts_with("__"))
            .unwrap_or(false)
        {
            return None;
        }
        world
            .get_component_by_id_as::<RaycastableComponent>(child)
            .copied()
    })
}

/// Graft (or remove) a layout-owned `RaycastableComponent` + `Quad2D` shape onto the
/// `__bg` renderable, mirroring the author's `Raycastable` settings. The bg quad's
/// scale already covers the padding box, so a unit `Quad2D` shape attached as a child
/// of the renderable hits the same world bounds as the visible surface.
fn sync_bg_author_raycastable(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    bg_id: ComponentId,
    author: Option<RaycastableComponent>,
) {
    let Some(renderable_id) = subtree_first_renderable(world, bg_id) else {
        return;
    };

    let existing_raycastable = world
        .children_of(renderable_id)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) == Some(OWNED_BG_RAYCASTABLE_LABEL)
                && world
                    .get_component_by_id_as::<RaycastableComponent>(child)
                    .is_some()
        });
    let existing_shape = world
        .children_of(renderable_id)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) == Some(OWNED_BG_RAYCASTABLE_SHAPE_LABEL)
                && world
                    .get_component_by_id_as::<RaycastableShapeComponent>(child)
                    .is_some()
        });

    match author {
        Some(rc) if rc.enable => {
            if let Some(rc_id) = existing_raycastable {
                if let Some(c) = world.get_component_by_id_as_mut::<RaycastableComponent>(rc_id) {
                    *c = rc;
                }
            } else {
                let rc_id =
                    world.add_component_boxed_named(OWNED_BG_RAYCASTABLE_LABEL, Box::new(rc));
                let _ = world.add_child(renderable_id, rc_id);
                world.init_component_tree(rc_id, emit);
            }

            if existing_shape.is_none() {
                let shape_id = world.add_component_boxed_named(
                    OWNED_BG_RAYCASTABLE_SHAPE_LABEL,
                    Box::new(RaycastableShapeComponent::new(RaycastableShapeType::Quad2D)),
                );
                let _ = world.add_child(renderable_id, shape_id);
                world.init_component_tree(shape_id, emit);
            }
        }
        _ => {
            if let Some(rc_id) = existing_raycastable {
                emit.push_intent_now(
                    rc_id,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![rc_id],
                    },
                );
            }
            if let Some(shape_id) = existing_shape {
                emit.push_intent_now(
                    shape_id,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![shape_id],
                    },
                );
            }
        }
    }
}

fn sync_scroll_drag_surface(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    bg_id: ComponentId,
    needs_scroll_drag_surface: bool,
) {
    let Some(renderable_id) = subtree_first_renderable(world, bg_id) else {
        return;
    };

    let existing_raycastable = world
        .children_of(renderable_id)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) == Some(OWNED_SCROLL_DRAG_RAYCASTABLE_LABEL)
                && world
                    .get_component_by_id_as::<RaycastableComponent>(child)
                    .is_some()
        });
    let existing_shape = world
        .children_of(renderable_id)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) == Some(OWNED_SCROLL_DRAG_SHAPE_LABEL)
                && world
                    .get_component_by_id_as::<RaycastableShapeComponent>(child)
                    .is_some()
        });

    if needs_scroll_drag_surface {
        if existing_raycastable.is_none() {
            let rc_id = world.add_component_boxed_named(
                OWNED_SCROLL_DRAG_RAYCASTABLE_LABEL,
                Box::new(RaycastableComponent::drag_only()),
            );
            let _ = world.add_child(renderable_id, rc_id);
            world.init_component_tree(rc_id, emit);
        }

        if existing_shape.is_none() {
            let shape_id = world.add_component_boxed_named(
                OWNED_SCROLL_DRAG_SHAPE_LABEL,
                Box::new(RaycastableShapeComponent::new(RaycastableShapeType::Quad2D)),
            );
            let _ = world.add_child(renderable_id, shape_id);
            world.init_component_tree(shape_id, emit);
        }
    } else {
        if let Some(rc_id) = existing_raycastable {
            emit.push_intent_now(
                rc_id,
                IntentValue::RemoveSubtree {
                    component_ids: vec![rc_id],
                },
            );
        }

        if let Some(shape_id) = existing_shape {
            emit.push_intent_now(
                shape_id,
                IntentValue::RemoveSubtree {
                    component_ids: vec![shape_id],
                },
            );
        }
    }
}

/// Spawn `__bg` → `ColorComponent` → `RenderableComponent` (+ optional `OpacityComponent`)
/// under `parent_tc_id` and initialise the subtree.
fn spawn_bg_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent_tc_id: ComponentId,
    rgba: [f32; 4],
) -> ComponentId {
    let bg_id = world.add_component_boxed_named("__bg", Box::new(TransformComponent::new()));
    let _ = world.add_child(parent_tc_id, bg_id);

    let serialize_id = world.add_component(SerializeComponent::off());
    let _ = world.add_child(bg_id, serialize_id);

    let color_id = world.add_component(ColorComponent { rgba });
    let _ = world.add_child(bg_id, color_id);

    let rend_id = world.add_component(RenderableComponent::square());
    let _ = world.add_child(color_id, rend_id);

    if rgba[3] < 1.0 {
        let op_id = world.add_component(OpacityComponent::new().with_opacity(rgba[3]));
        let _ = world.add_child(rend_id, op_id);
    }

    world.init_component_tree(bg_id, emit);
    bg_id
}

#[cfg(test)]
mod tests {
    use crate::engine::ecs::component::style::{EdgeInsets, SizeDimension};
    use crate::engine::ecs::component::{
        ColorComponent, LayoutComponent, SerializeComponent, StencilClipComponent, StyleComponent,
        TextComponent, TransformComponent,
    };
    use crate::engine::ecs::system::layout::LayoutSystem;
    use crate::engine::ecs::{CommandQueue, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn block_layout_recurses_into_styled_container_children() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(12.0));

        let container =
            world.add_component_boxed_named("container", Box::new(TransformComponent::new()));
        let container_style = world.add_component({
            let mut style = StyleComponent::new();
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(6.0);
            style.margin = EdgeInsets::all(1.0);
            style.padding = EdgeInsets::all(2.0);
            style
        });

        let item = world.add_component_boxed_named("item", Box::new(TransformComponent::new()));
        let item_style = world.add_component({
            let mut style = StyleComponent::new();
            style.margin = EdgeInsets::all(0.5);
            style.padding = EdgeInsets::all(0.25);
            style.background_color = Some([1.0, 0.0, 0.0, 1.0]);
            style
        });
        let text = world.add_component(TextComponent::new("hello"));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));

        let _ = world.add_child(root, container);
        let _ = world.add_child(container, container_style);
        let _ = world.add_child(container, item);
        let _ = world.add_child(item, item_style);
        let _ = world.add_child(item, text);
        let _ = world.add_child(text, color);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let item_tc = world
            .get_component_by_id_as::<TransformComponent>(item)
            .expect("item transform");

        assert_eq!(item_tc.transform.translation, [0.75, -0.75, 0.0]);
        assert!(
            world
                .children_of(item)
                .iter()
                .any(|&child| world.component_label(child) == Some("__bg"))
        );
    }

    #[test]
    fn block_layout_marks_owned_background_quad_serialize_off() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(8.0));
        let item = world.add_component_boxed_named("item", Box::new(TransformComponent::new()));
        let item_style = world.add_component({
            let mut style = StyleComponent::new();
            style.width = SizeDimension::GlyphUnits(6.0);
            style.height = SizeDimension::GlyphUnits(2.0);
            style.background_color = Some([0.2, 0.3, 0.4, 1.0]);
            style
        });

        let _ = world.add_child(root, item);
        let _ = world.add_child(item, item_style);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let bg = world
            .children_of(item)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some("__bg"))
            .expect("expected layout-owned __bg child");
        let serialize = world
            .children_of(bg)
            .iter()
            .copied()
            .find(|&child| {
                world
                    .get_component_by_id_as::<SerializeComponent>(child)
                    .is_some()
            })
            .expect("expected serialize marker on __bg");
        assert!(
            world
                .get_component_by_id_as::<SerializeComponent>(serialize)
                .is_some_and(|marker| !marker.enabled)
        );
    }

    #[test]
    fn block_layout_updates_existing_background_quad_color_when_style_changes() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(8.0));
        let item = world.add_component_boxed_named("item", Box::new(TransformComponent::new()));
        let item_style = world.add_component({
            let mut style = StyleComponent::new();
            style.width = SizeDimension::GlyphUnits(6.0);
            style.height = SizeDimension::GlyphUnits(2.0);
            style.background_color = Some([0.2, 0.3, 0.4, 1.0]);
            style
        });

        let _ = world.add_child(root, item);
        let _ = world.add_child(item, item_style);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let bg = world
            .children_of(item)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some("__bg"))
            .expect("expected layout-owned __bg child");
        let color_id = world
            .children_of(bg)
            .iter()
            .copied()
            .find(|&child| {
                world
                    .get_component_by_id_as::<ColorComponent>(child)
                    .is_some()
            })
            .expect("expected color child on __bg");

        if let Some(style) = world.get_component_by_id_as_mut::<StyleComponent>(item_style) {
            style.background_color = Some([0.9, 0.8, 0.2, 1.0]);
        }

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        assert_eq!(
            world
                .get_component_by_id_as::<ColorComponent>(color_id)
                .expect("bg color")
                .rgba,
            [0.9, 0.8, 0.2, 1.0]
        );
    }

    #[test]
    fn block_layout_does_not_reflow_unstyled_decorative_children() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(8.0));

        let header_slot =
            world.add_component_boxed_named("header_slot", Box::new(TransformComponent::new()));
        let header_style = world.add_component({
            let mut style = StyleComponent::new();
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(2.0);
            style
        });

        let title_bar = world.add_component_boxed_named(
            "panel_titlebar_t",
            Box::new(
                TransformComponent::new()
                    .with_position(10.0, -1.0, 0.005)
                    .with_scale(20.0, 2.0, 1.0),
            ),
        );
        let title_label = world.add_component_boxed_named(
            "panel_titlebar_label_t",
            Box::new(
                TransformComponent::new()
                    .with_position(0.02, -0.04, 0.01)
                    .with_scale(0.08, 0.08, 0.08),
            ),
        );
        let title_text = world.add_component(TextComponent::new("World"));
        let title_color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));

        let _ = world.add_child(root, header_slot);
        let _ = world.add_child(header_slot, header_style);
        let _ = world.add_child(header_slot, title_bar);
        let _ = world.add_child(header_slot, title_label);
        let _ = world.add_child(title_label, title_color);
        let _ = world.add_child(title_color, title_text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let title_bar_tc = world
            .get_component_by_id_as::<TransformComponent>(title_bar)
            .expect("title bar transform");
        let title_label_tc = world
            .get_component_by_id_as::<TransformComponent>(title_label)
            .expect("title label transform");

        assert_eq!(title_bar_tc.transform.translation, [10.0, -1.0, 0.005]);
        assert_eq!(title_label_tc.transform.translation, [0.02, -0.04, 0.01]);
    }

    #[test]
    fn text_align_scales_offsets_by_layout_unit_scale() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));

        let button = world.add_component_boxed_named("button", Box::new(TransformComponent::new()));
        let button_style = world.add_component({
            let mut style = StyleComponent::new();
            style.width = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(6.0);
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(2.0);
            style.text_align = crate::engine::ecs::component::style::TextAlign::Center;
            // 0.08 wu per glyph quad — under unit_scale=0.08 this is the
            // "1 row per GU" canonical sizing.
            style.font_size = crate::engine::ecs::component::style::SizeDimension::WorldUnits(0.08);
            style
        });
        let text_wrap = world.add_component_boxed_named(
            "text_wrap",
            Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.05)),
        );
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("Save")));

        let _ = world.add_child(root, button);
        let _ = world.add_child(button, button_style);
        let _ = world.add_child(button, text_wrap);
        let _ = world.add_child(text_wrap, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let text_wrap_tc = world
            .get_component_by_id_as::<TransformComponent>(text_wrap)
            .expect("text_wrap transform");

        assert!((text_wrap_tc.transform.translation[0] - 0.08).abs() < 1e-4);
        assert!((text_wrap_tc.transform.translation[1] + 0.04).abs() < 1e-4);
        assert!((text_wrap_tc.transform.translation[2] - 0.05).abs() < 1e-4);
    }

    #[test]
    fn vertical_align_top_overrides_legacy_centering() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));

        let button = world.add_component_boxed_named("button", Box::new(TransformComponent::new()));
        let button_style = world.add_component({
            let mut style = StyleComponent::new();
            style.width = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(6.0);
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(2.0);
            style.text_align = crate::engine::ecs::component::style::TextAlign::Center;
            style.vertical_align = crate::engine::ecs::component::style::VerticalAlign::Top;
            style.font_size = crate::engine::ecs::component::style::SizeDimension::WorldUnits(0.08);
            style
        });
        let text_wrap = world.add_component_boxed_named(
            "text_wrap",
            Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.05)),
        );
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("Save")));

        let _ = world.add_child(root, button);
        let _ = world.add_child(button, button_style);
        let _ = world.add_child(button, text_wrap);
        let _ = world.add_child(text_wrap, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let text_wrap_tc = world
            .get_component_by_id_as::<TransformComponent>(text_wrap)
            .expect("text_wrap transform");

        assert!((text_wrap_tc.transform.translation[0] - 0.08).abs() < 1e-4);
        assert!(text_wrap_tc.transform.translation[1].abs() < 1e-4);
        assert!((text_wrap_tc.transform.translation[2] - 0.05).abs() < 1e-4);
    }

    #[test]
    fn vertical_align_middle_respects_text_font_size() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));

        let button = world.add_component_boxed_named("button", Box::new(TransformComponent::new()));
        let button_style = world.add_component({
            let mut style = StyleComponent::new();
            style.width = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(6.875);
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(2.4);
            style.text_align = crate::engine::ecs::component::style::TextAlign::Center;
            style.vertical_align = crate::engine::ecs::component::style::VerticalAlign::Middle;
            style.font_size = crate::engine::ecs::component::style::SizeDimension::WorldUnits(0.08);
            style
        });
        let text_wrap = world.add_component_boxed_named(
            "text_wrap",
            Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.05)),
        );
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("Save")));

        let _ = world.add_child(root, button);
        let _ = world.add_child(button, button_style);
        let _ = world.add_child(button, text_wrap);
        let _ = world.add_child(text_wrap, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let text_wrap_tc = world
            .get_component_by_id_as::<TransformComponent>(text_wrap)
            .expect("text_wrap transform");

        // Box content width = 6.875 GU = 0.55 wu under unit_scale=0.08.
        // "Save" measures to 4 columns × 0.08 wu = 0.32 wu. Center inset:
        //   x = (0.55 - 0.32) / 2 = 0.115 wu
        // Vertical: content_h = 2.4 GU = 0.192 wu; text_h = 0.08 wu →
        //   y = -((0.192 - 0.08) / 2) = -0.056 wu
        assert!((text_wrap_tc.transform.translation[0] - 0.115).abs() < 1e-4);
        assert!((text_wrap_tc.transform.translation[1] + 0.056).abs() < 1e-4);
        assert!((text_wrap_tc.transform.translation[2] - 0.05).abs() < 1e-4);
    }

    #[test]
    fn auto_aligned_text_uses_content_box_top_left_origin() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));

        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component({
            let mut style = StyleComponent::new();
            style.font_size = crate::engine::ecs::component::style::SizeDimension::WorldUnits(0.08);
            style.padding = EdgeInsets::all(0.45);
            style
        });
        let text_wrap = world.add_component_boxed_named(
            "text_wrap",
            Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.05)),
        );
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("idle")));

        let _ = world.add_child(root, row);
        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, text_wrap);
        let _ = world.add_child(text_wrap, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let text_wrap_tc = world
            .get_component_by_id_as::<TransformComponent>(text_wrap)
            .expect("text_wrap transform");

        // Text wrapper origin now matches the content-box top-left; glyph
        // quads remain center-origin'd inside that wrapper.
        assert!(text_wrap_tc.transform.translation[0].abs() < 1e-4);
        assert!(text_wrap_tc.transform.translation[1].abs() < 1e-4);
        assert!((text_wrap_tc.transform.translation[2] - 0.05).abs() < 1e-4);
    }

    #[test]
    fn overflow_scroll_uses_sibling_layout_owned_stencil_clip() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(8.0));
        let item =
            world.add_component_boxed_named("scroll_item", Box::new(TransformComponent::new()));
        let style = world.add_component({
            let mut style = StyleComponent::new();
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(4.0);
            style.background_color = Some([0.2, 0.2, 0.2, 1.0]);
            style.overflow = crate::engine::ecs::component::Overflow::Scroll;
            style
        });

        let _ = world.add_child(root, item);
        let _ = world.add_child(item, style);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let bg = world
            .children_of(item)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some("__bg"));
        let clip = world.children_of(item).iter().copied().find(|&child| {
            world.component_label(child) == Some(super::OWNED_LAYOUT_STENCIL_CLIP_LABEL)
                && world
                    .get_component_by_id_as::<StencilClipComponent>(child)
                    .is_some()
        });

        assert!(bg.is_some(), "expected layout-owned __bg child");
        assert!(clip.is_some(), "expected sibling layout-owned stencil clip");
        assert_eq!(world.parent_of(clip.expect("clip")), Some(item));
    }

    #[test]
    fn nested_inline_child_background_clamps_to_layoutroot_width_after_layout() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(9.5).with_height(12.0));

        let title_bar =
            world.add_component_boxed_named("title_bar", Box::new(TransformComponent::new()));
        let title_bar_style = world.add_component({
            let mut style = StyleComponent::new();
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(3.0);
            style.background_color = Some([0.2, 0.2, 0.2, 1.0]);
            style
        });

        let make_inline_box =
            |world: &mut World, name: &'static str, width_gu: f32, color: [f32; 4]| {
                let tc = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
                let style = world.add_component({
                    let mut s = StyleComponent::new();
                    s.display = Some(crate::engine::ecs::component::style::Display::InlineBlock);
                    s.width =
                        crate::engine::ecs::component::style::SizeDimension::GlyphUnits(width_gu);
                    s.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(3.0);
                    s.background_color = Some(color);
                    s
                });
                let _ = world.add_child(tc, style);
                tc
            };

        let title = make_inline_box(&mut world, "title", 14.5, [0.9, 0.2, 0.2, 1.0]);
        let save = make_inline_box(&mut world, "save", 6.875, [0.2, 0.9, 0.2, 1.0]);
        let load = make_inline_box(&mut world, "load", 6.875, [0.2, 0.2, 0.9, 1.0]);

        let _ = world.add_child(root, title_bar);
        let _ = world.add_child(title_bar, title_bar_style);
        let _ = world.add_child(title_bar, title);
        let _ = world.add_child(title_bar, save);
        let _ = world.add_child(title_bar, load);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let child_bg = |world: &World, owner| {
            world
                .children_of(owner)
                .iter()
                .copied()
                .find(|&child| world.component_label(child) == Some("__bg"))
        };

        let title_bg = child_bg(&world, title).expect("title bg");
        let save_bg = child_bg(&world, save).expect("save bg");
        let load_bg = child_bg(&world, load).expect("load bg");
        let title_bg_tc = world
            .get_component_by_id_as::<TransformComponent>(title_bg)
            .expect("title bg transform");
        let save_bg_tc = world
            .get_component_by_id_as::<TransformComponent>(save_bg)
            .expect("save bg transform");
        let load_bg_tc = world
            .get_component_by_id_as::<TransformComponent>(load_bg)
            .expect("load bg transform");
        let title_tc = world
            .get_component_by_id_as::<TransformComponent>(title)
            .expect("title transform");
        let save_tc = world
            .get_component_by_id_as::<TransformComponent>(save)
            .expect("save transform");
        let load_tc = world
            .get_component_by_id_as::<TransformComponent>(load)
            .expect("load transform");

        assert!((title_bg_tc.transform.scale[0] - 9.5).abs() < 1e-4);
        assert!(save_bg_tc.transform.scale[0] <= 9.5 + 1e-4);
        assert!(load_bg_tc.transform.scale[0] <= 9.5 + 1e-4);
        assert_eq!(title_tc.transform.translation[0], 0.0);
        assert_eq!(save_tc.transform.translation[0], 0.0);
        assert_eq!(load_tc.transform.translation[0], 0.0);
        assert!(save_tc.transform.translation[1] < title_tc.transform.translation[1]);
        assert!(load_tc.transform.translation[1] < save_tc.transform.translation[1]);
    }

    #[test]
    fn inline_overflow_hidden_creates_clipped_content_root() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(12.0));

        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component({
            let mut style = StyleComponent::new();
            style.background_color = Some([0.2, 0.2, 0.2, 1.0]);
            style
        });
        let _ = world.add_child(root, row);
        let _ = world.add_child(row, row_style);

        let chip = world.add_component_boxed_named("chip", Box::new(TransformComponent::new()));
        let chip_style = world.add_component({
            let mut s = StyleComponent::new();
            s.display = Some(crate::engine::ecs::component::style::Display::InlineBlock);
            s.width = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(8.0);
            s.background_color = Some([0.9, 0.8, 0.2, 1.0]);
            s.overflow = crate::engine::ecs::component::Overflow::Hidden;
            s
        });
        let text_wrapper =
            world.add_component_boxed_named("text_wrapper", Box::new(TransformComponent::new()));
        let text = world.add_component_boxed_named(
            "text",
            Box::new(TextComponent::new("inline 1.6 inline 1.6")),
        );

        let _ = world.add_child(row, chip);
        let _ = world.add_child(chip, chip_style);
        let _ = world.add_child(chip, text_wrapper);
        let _ = world.add_child(text_wrapper, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let clipped = world.children_of(chip).iter().copied().find(|&child| {
            world.component_label(child) == Some(super::OWNED_CLIPPED_CONTENT_LABEL)
                && world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .is_some()
        });
        let clip = world.children_of(chip).iter().copied().find(|&child| {
            world.component_label(child) == Some(super::OWNED_LAYOUT_STENCIL_CLIP_LABEL)
                && world
                    .get_component_by_id_as::<StencilClipComponent>(child)
                    .is_some()
        });

        assert!(
            clipped.is_some(),
            "expected inline overflow-hidden item to get clipped content root"
        );
        assert!(
            clip.is_some(),
            "expected inline overflow-hidden item to get stencil clip sibling"
        );
        assert_eq!(
            world.parent_of(text_wrapper),
            clipped,
            "authored inline content should move under clipped content root"
        );
    }

    /// Multi-line text under `vertical_align: middle` and `unit_scale != 1.0`
    /// must center the **rendered** text block (rows × font_size in world units),
    /// not the GU-claimed block. Previously the math mixed GU and WU and only
    /// happened to work for single-line text. With three lines under a panel
    /// `unit_scale = 0.08` and `font_size = 1gu`, the rendered block is
    /// `3 * 0.08 = 0.24 wu`, the content box is `4 GU * 0.08 = 0.32 wu`, and
    /// the inner T should sit at `y = -((0.32 - 0.24) / 2) = -0.04 wu`.
    #[test]
    fn vertical_align_middle_centers_multi_line_text_under_unit_scale() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));

        let box_tc = world.add_component_boxed_named("box", Box::new(TransformComponent::new()));
        let box_style = world.add_component({
            let mut style = StyleComponent::new();
            style.width = SizeDimension::GlyphUnits(10.0);
            style.height = SizeDimension::GlyphUnits(4.0);
            style.text_align = crate::engine::ecs::component::style::TextAlign::Left;
            style.vertical_align = crate::engine::ecs::component::style::VerticalAlign::Middle;
            // 1 row per GU — the canonical "GU = row" sizing.
            style.font_size = SizeDimension::GlyphUnits(1.0);
            style
        });
        let text_wrap = world.add_component_boxed_named(
            "text_wrap",
            Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.05)),
        );
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("a\nb\nc")));
        let _ = world.add_child(root, box_tc);
        let _ = world.add_child(box_tc, box_style);
        let _ = world.add_child(box_tc, text_wrap);
        let _ = world.add_child(text_wrap, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);
        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let text_wrap_tc = world
            .get_component_by_id_as::<TransformComponent>(text_wrap)
            .expect("text_wrap transform");

        // Verify y centers the rendered three-row block. content_h_wu = 0.32,
        // text_h_wu = 3*0.08 = 0.24.
        assert!(
            (text_wrap_tc.transform.translation[1] + 0.04).abs() < 1e-4,
            "expected y ≈ -0.04, got {}",
            text_wrap_tc.transform.translation[1]
        );
    }

    /// `Style.font_size` may be authored in glyph units. Under `unit_scale = 0.08`,
    /// `1gu` resolves to a 0.08 wu glyph quad — what the renderer actually
    /// scales by.
    #[test]
    fn style_font_size_in_glyph_units_resolves_to_world_units_via_unit_scale() {
        use crate::engine::ecs::component::TextComponent;
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component({
            let mut s = StyleComponent::new();
            s.font_size = SizeDimension::GlyphUnits(1.0);
            s.padding = crate::engine::ecs::component::style::EdgeInsets::all(0.5);
            s
        });
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("hi")));
        let _ = world.add_child(root, row);
        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);
        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let stamped = world
            .get_component_by_id_as::<TextComponent>(text)
            .expect("text component")
            .font_size;
        assert!(
            (stamped - 0.08).abs() < 1e-6,
            "GlyphUnits(1.0) under unit_scale=0.08 should stamp 0.08 wu, got {stamped}"
        );
    }

    /// `Style.font_size` may also be authored in world units. The wu value
    /// passes through to the descendant `TextComponent` unchanged regardless
    /// of the enclosing `unit_scale`.
    #[test]
    fn style_font_size_in_world_units_passes_through_unchanged() {
        use crate::engine::ecs::component::TextComponent;
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component({
            let mut s = StyleComponent::new();
            s.font_size = SizeDimension::WorldUnits(0.12);
            s
        });
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("hi")));
        let _ = world.add_child(root, row);
        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, text);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);
        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut queue);

        let stamped = world
            .get_component_by_id_as::<TextComponent>(text)
            .expect("text component")
            .font_size;
        assert!(
            (stamped - 0.12).abs() < 1e-6,
            "WorldUnits(0.12) should stamp 0.12 wu directly, got {stamped}"
        );
    }

    /// Regression for the `world_panel_content` row bug: an auto-height styled
    /// box with a single line of text and `font_size = 1gu` (canonical "one row
    /// per glyph unit") must reserve a content area whose height equals the
    /// rendered glyph height. Pre-refactor the content_height was
    /// `rows * font_size` in *mixed* units and ended up 12.5× too small under
    /// `unit_scale = 0.08`, so glyphs spilled into the padding/margin band.
    #[test]
    fn auto_height_styled_row_contains_rendered_text_under_unit_scale() {
        use crate::engine::ecs::component::TextComponent;
        let mut world = World::default();

        let _root = world.add_component(LayoutComponent::new(20.0).with_unit_scale(0.08));
        let row = world.add_component_boxed_named("row", Box::new(TransformComponent::new()));
        let row_style = world.add_component({
            let mut s = StyleComponent::new();
            s.font_size = SizeDimension::GlyphUnits(1.0);
            s
        });
        let text = world.add_component_boxed_named("text", Box::new(TextComponent::new("idle")));
        let _ = world.add_child(row, row_style);
        let _ = world.add_child(row, text);

        let measured = super::super::measure::measure_item(&world, row, 20.0, None, 0.08);
        assert!(
            (measured.content_height_gu - 1.0).abs() < 1e-4,
            "expected content_height_gu = 1.0 (one row per GU), got {}",
            measured.content_height_gu
        );
    }
}
