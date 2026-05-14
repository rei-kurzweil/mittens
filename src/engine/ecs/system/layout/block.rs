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
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::{IntentValue, SignalEmitter};
use crate::engine::ecs::component::{
    ColorComponent, OpacityComponent, Overflow, RenderableComponent, RouterComponent,
    ScrollingComponent, StencilClipComponent,
    RaycastableComponent, RaycastableShapeComponent, RaycastableShapeType,
    StyleComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::system::ScrollingSystem;
use super::measure::{apply_text_color_for_item, apply_text_wrap_for_item, measure_container_items, measure_items, MeasuredItem};
use crate::engine::ecs::component::style::{Display, TextAlign};
use crate::engine::ecs::system::text_system::TextSystem;

const OWNED_CLIPPED_CONTENT_LABEL: &str = "__clip_content";
const OWNED_LAYOUT_STENCIL_CLIP_LABEL: &str = "__layout_stencil_clip";
const OWNED_LAYOUT_OVERFLOW_ROUTER_LABEL: &str = "__layout_overflow_router";
const OWNED_SCROLL_WRAPPER_LABEL: &str = "__scroll";
const OWNED_SCROLL_DRAG_RAYCASTABLE_LABEL: &str = "__scroll_drag_raycastable";
const OWNED_SCROLL_DRAG_SHAPE_LABEL: &str = "__scroll_drag_shape";
const OWNED_BG_RAYCASTABLE_LABEL: &str = "__bg_raycastable";
const OWNED_BG_RAYCASTABLE_SHAPE_LABEL: &str = "__bg_raycastable_shape";

/// Run a block formatting context layout pass for `layout_id`.
///
/// Calls `measure_items` (Pass 1) then walks the results with a vertical cursor,
/// emits `UpdateTransform` for each TC child, and manages background quads for
/// items with `Style { background_color }`.
pub fn layout(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    layout_id: ComponentId,
) {
    let (items, _avail_w, _avail_h, unit_scale) = measure_items(world, layout_id);

    layout_items(world, emit, &items, unit_scale);
}

/// Public-to-the-layout-module entry so `inline::layout_items` can recurse
/// back into block flow when an inline-block item's children are block-level.
pub(crate) fn layout_items_for(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    items: &[MeasuredItem],
    unit_scale: f32,
) {
    layout_items(world, emit, items, unit_scale);
}

fn layout_items(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    items: &[MeasuredItem],
    unit_scale: f32,
) {

    let mut cursor_gu = 0.0_f32;

    for item in items {
        cursor_gu += item.margin_top_gu;

        let content_origin_y_gu = cursor_gu + item.padding_top_gu;
        let content_origin_x_gu = item.margin_left_gu + item.padding_left_gu;

        // Preserve the TC's existing local Z and scale — LayoutSystem controls X/Y only.
        let (tc_scale, tc_z) = world
            .get_component_by_id_as::<TransformComponent>(item.tc_id)
            .map(|tc| (tc.transform.scale, tc.transform.translation[2]))
            .unwrap_or(([1.0, 1.0, 1.0], 0.0));

        emit.push_intent_now(
            item.tc_id,
            IntentValue::UpdateTransform {
                component_ids: vec![item.tc_id],
                translation: [
                      content_origin_x_gu * unit_scale,
                    -(content_origin_y_gu * unit_scale),
                                        tc_z,
                ],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: tc_scale,
            },
        );

        // Push the container-derived wrap_at into any descendant TextComponent
        // and rebuild glyphs so the rendered text matches the measured width.
        apply_text_wrap_for_item(world, emit, item.tc_id, item.content_width_gu);
        apply_text_color_for_item(world, emit, item.tc_id);

        // ── Background quad / overflow helper topology ───────────────────
        sync_bg_quad(world, emit, item.tc_id, item.padding_left_gu, item.padding_top_gu, item.box_width_gu, item.box_height_gu, unit_scale);
        apply_text_align(world, emit, item.tc_id, item.content_width_gu, item.content_height_gu);
        let content_root = sync_overflow_topology(world, emit, item.tc_id, item.content_height_gu);

        let nested_items = measure_container_items(
            world,
            content_root,
            item.content_width_gu,
            Some(item.content_height_gu),
        );
        if let Some(scroll_id) = immediate_owned_scroll_wrapper(world, item.tc_id) {
            sync_scrolling_metrics(world, emit, scroll_id, item.content_height_gu, &nested_items);
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
            if all_inline_block {
                super::inline::layout_items(
                    world,
                    emit,
                    &nested_items,
                    item.content_width_gu,
                    unit_scale,
                );
            } else {
                layout_items(world, emit, &nested_items, unit_scale);
            }
        }

        cursor_gu += item.box_height_gu + item.margin_bottom_gu;
    }
}

fn style_overflow(world: &World, tc_id: ComponentId) -> Overflow {
    world.children_of(tc_id).iter().find_map(|&child| {
        world
            .get_component_by_id_as::<StyleComponent>(child)
            .map(|style| style.overflow)
    }).unwrap_or(Overflow::Visible)
}

fn immediate_owned_layout_router(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_LAYOUT_OVERFLOW_ROUTER_LABEL)
            && world.get_component_by_id_as::<RouterComponent>(child).is_some()
    })
}

fn immediate_owned_clipped_content(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_CLIPPED_CONTENT_LABEL)
            && world.get_component_by_id_as::<TransformComponent>(child).is_some()
    })
}

fn immediate_owned_scroll_wrapper(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_SCROLL_WRAPPER_LABEL)
            && world.get_component_by_id_as::<ScrollingComponent>(child).is_some()
    })
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
        return scroll_id;
    }

    let scroll_id = world.add_component_boxed_named(
        OWNED_SCROLL_WRAPPER_LABEL,
        Box::new(ScrollingComponent::new(viewport_height.max(0.0), viewport_height.max(0.0))),
    );
    let _ = world.add_child(owner, scroll_id);
    world.init_component_tree(scroll_id, emit);
    scroll_id
}

fn authored_overflow_children(world: &World, owner: ComponentId) -> Vec<ComponentId> {
    world
        .children_of(owner)
        .iter()
        .copied()
        .filter(|&child| {
            world.get_component_by_id_as::<TransformComponent>(child).is_some()
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
    world
        .get_component_by_id_as::<ScrollingComponent>(scroll_id)
        .and_then(|sc| sc.track)
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

fn sync_overflow_topology(
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
            relocate_authored_children(world, tc_id, scroll_id);
            scroll_content_root(world, scroll_id)
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
        world.get_component_by_id_as::<StyleComponent>(ch)
            .map(|s| (s.background_color, s.background_z, s.overflow))
    });

    let existing_bg = children.iter()
        .find(|&&ch| world.component_label(ch) == Some("__bg"))
        .copied();

    let (needs_clip, needs_scroll_drag_surface, bg_spec) = match bg_style {
        Some((rgba, bg_z, overflow)) => (
            matches!(overflow, Overflow::Hidden | Overflow::Scroll),
            matches!(overflow, Overflow::Scroll),
            Some((rgba, bg_z)),
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

            emit.push_intent_now(
                bg_id,
                IntentValue::UpdateTransform {
                    component_ids: vec![bg_id],
                    translation: [
                        (box_width_gu / 2.0 - padding_left_gu - 0.5) * unit_scale,
                        (padding_top_gu - box_height_gu / 2.0 + 0.5) * unit_scale,
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
            IntentValue::RemoveSubtree { component_ids: vec![bg_id] },
        );
    }
}

fn immediate_owned_layout_stencil_clip(world: &World, scope_root: ComponentId) -> Option<ComponentId> {
    world.children_of(scope_root).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_LAYOUT_STENCIL_CLIP_LABEL)
            && world.get_component_by_id_as::<StencilClipComponent>(child).is_some()
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
            IntentValue::RemoveSubtree { component_ids: vec![clip_id] },
        );
    }
}

fn subtree_first_renderable(world: &World, root: ComponentId) -> Option<ComponentId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
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
/// of the content area. When the author sets `text_align`, locate the first
/// direct-child T whose subtree contains a `TextComponent`, measure the text,
/// and emit `UpdateTransform` so the glyph block sits aligned horizontally per
/// the alignment value and **always vertically centered** within the content
/// box. `TextAlign::Auto` (default) preserves the author's translation.
fn apply_text_align(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
    content_w_gu: f32,
    content_h_gu: f32,
) {
    let style = world.children_of(tc_id).iter().find_map(|&ch| {
        world.get_component_by_id_as::<StyleComponent>(ch).map(|s| (s.text_align, s.word_wrap, s.word_wrap_tokens.clone()))
    });
    let Some((align, _style_wrap, _style_tokens)) = style else { return; };
    if align == TextAlign::Auto {
        return;
    }

    let Some(inner_tc) = find_text_bearing_direct_child(world, tc_id) else {
        return;
    };

    // Measure text directly off the inner T's TextComponent so we get the
    // post-wrap shape (callers of layout run `apply_text_wrap_for_item` later,
    // but glyphs build with the authored wrap_at and we want the natural-
    // wrap width here — i.e. no wrap — to drive alignment math).
    let (text, word_wrap, tokens) = match find_text_descriptor(world, inner_tc) {
        Some(t) => t,
        None => return,
    };
    let (max_col, line_count) = TextSystem::measure(&text, 0, word_wrap, &tokens);
    let text_w = max_col as f32;
    let text_h = line_count as f32;

    // Glyphs are 1×1 quads centered at column / row positions. The leftmost
    // glyph's center sits at x = inner_tc.x and spans [-0.5, +0.5] around it,
    // so adding 0.5 to the alignment offset puts the left edge of the first
    // glyph at content_x = 0. Same logic on y (row 0 center at y = inner_tc.y,
    // glyph spans [-0.5, +0.5]; top edge at y = 0 needs y_offset of -0.5).
    let x_offset = match align {
        TextAlign::Left  => 0.5,
        TextAlign::Right => (content_w_gu - text_w + 0.5).max(0.5),
        TextAlign::Center => (content_w_gu - text_w + 1.0) / 2.0,
        TextAlign::Auto => 0.0,
    };
    let y_offset = -((content_h_gu - text_h + 1.0) / 2.0);

    let (scale, z) = world
        .get_component_by_id_as::<TransformComponent>(inner_tc)
        .map(|tc| (tc.transform.scale, tc.transform.translation[2]))
        .unwrap_or(([1.0, 1.0, 1.0], 0.0));

    emit.push_intent_now(
        inner_tc,
        IntentValue::UpdateTransform {
            component_ids: vec![inner_tc],
            translation: [x_offset, y_offset, z],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale,
        },
    );
}

fn find_text_bearing_direct_child(world: &World, tc_id: ComponentId) -> Option<ComponentId> {
    for &child in world.children_of(tc_id) {
        if world.component_label(child).map(|l| l.starts_with("__")).unwrap_or(false) {
            continue;
        }
        if world.get_component_by_id_as::<TransformComponent>(child).is_none() {
            continue;
        }
        if subtree_has_text(world, child) {
            return Some(child);
        }
    }
    None
}

fn subtree_has_text(world: &World, root: ComponentId) -> bool {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if world.get_component_by_id_as::<TextComponent>(node).is_some() {
            return true;
        }
        for &child in world.children_of(node) {
            // Don't descend into another styled layout item.
            if world.get_component_by_id_as::<StyleComponent>(child).is_some() && child != root {
                continue;
            }
            stack.push(child);
        }
    }
    false
}

fn find_text_descriptor(world: &World, root: ComponentId) -> Option<(String, bool, Vec<String>)> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(t) = world.get_component_by_id_as::<TextComponent>(node) {
            return Some((t.text.clone(), t.word_wrap, t.word_wrap_tokens.clone()));
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
        if world.component_label(child).map(|l| l.starts_with("__")).unwrap_or(false) {
            return None;
        }
        world.get_component_by_id_as::<RaycastableComponent>(child).copied()
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

    let existing_raycastable = world.children_of(renderable_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_BG_RAYCASTABLE_LABEL)
            && world.get_component_by_id_as::<RaycastableComponent>(child).is_some()
    });
    let existing_shape = world.children_of(renderable_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_BG_RAYCASTABLE_SHAPE_LABEL)
            && world.get_component_by_id_as::<RaycastableShapeComponent>(child).is_some()
    });

    match author {
        Some(rc) if rc.enable => {
            if let Some(rc_id) = existing_raycastable {
                if let Some(c) = world.get_component_by_id_as_mut::<RaycastableComponent>(rc_id) {
                    *c = rc;
                }
            } else {
                let rc_id = world.add_component_boxed_named(
                    OWNED_BG_RAYCASTABLE_LABEL,
                    Box::new(rc),
                );
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
                    IntentValue::RemoveSubtree { component_ids: vec![rc_id] },
                );
            }
            if let Some(shape_id) = existing_shape {
                emit.push_intent_now(
                    shape_id,
                    IntentValue::RemoveSubtree { component_ids: vec![shape_id] },
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

    let existing_raycastable = world.children_of(renderable_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_SCROLL_DRAG_RAYCASTABLE_LABEL)
            && world.get_component_by_id_as::<RaycastableComponent>(child).is_some()
    });
    let existing_shape = world.children_of(renderable_id).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_SCROLL_DRAG_SHAPE_LABEL)
            && world.get_component_by_id_as::<RaycastableShapeComponent>(child).is_some()
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
                IntentValue::RemoveSubtree { component_ids: vec![rc_id] },
            );
        }

        if let Some(shape_id) = existing_shape {
            emit.push_intent_now(
                shape_id,
                IntentValue::RemoveSubtree { component_ids: vec![shape_id] },
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
    use crate::engine::ecs::component::{ColorComponent, LayoutComponent, StencilClipComponent, StyleComponent, TextComponent, TransformComponent};
    use crate::engine::ecs::component::style::EdgeInsets;
    use crate::engine::ecs::{CommandQueue, SystemWorld, World};
    use crate::engine::graphics::VisualWorld;
    use crate::engine::ecs::system::layout::LayoutSystem;

    #[test]
    fn block_layout_recurses_into_styled_container_children() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(12.0));

        let container = world.add_component_boxed_named("container", Box::new(TransformComponent::new()));
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
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let item_tc = world
            .get_component_by_id_as::<TransformComponent>(item)
            .expect("item transform");

        assert_eq!(item_tc.transform.translation, [0.75, -0.75, 0.0]);
        assert!(world.children_of(item).iter().any(|&child| world.component_label(child) == Some("__bg")));
    }

    #[test]
    fn block_layout_does_not_reflow_unstyled_decorative_children() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(8.0));

        let header_slot = world.add_component_boxed_named("header_slot", Box::new(TransformComponent::new()));
        let header_style = world.add_component({
            let mut style = StyleComponent::new();
            style.height = crate::engine::ecs::component::style::SizeDimension::GlyphUnits(2.0);
            style
        });

        let title_bar = world.add_component_boxed_named(
            "panel_titlebar_t",
            Box::new(TransformComponent::new().with_position(10.0, -1.0, 0.005).with_scale(20.0, 2.0, 1.0)),
        );
        let title_label = world.add_component_boxed_named(
            "panel_titlebar_label_t",
            Box::new(TransformComponent::new().with_position(0.02, -0.04, 0.01).with_scale(0.08, 0.08, 0.08)),
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
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

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
    fn overflow_scroll_uses_sibling_layout_owned_stencil_clip() {
        let mut world = World::default();
        let mut visuals = VisualWorld::new();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();
        let mut layout_system = LayoutSystem::new();

        let root = world.add_component(LayoutComponent::new(20.0).with_height(8.0));
        let item = world.add_component_boxed_named("scroll_item", Box::new(TransformComponent::new()));
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
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        layout_system.tick(&mut world, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let bg = world.children_of(item).iter().copied().find(|&child| world.component_label(child) == Some("__bg"));
        let clip = world.children_of(item).iter().copied().find(|&child| {
            world.component_label(child) == Some(super::OWNED_LAYOUT_STENCIL_CLIP_LABEL)
                && world.get_component_by_id_as::<StencilClipComponent>(child).is_some()
        });

        assert!(bg.is_some(), "expected layout-owned __bg child");
        assert!(clip.is_some(), "expected sibling layout-owned stencil clip");
        assert_eq!(world.parent_of(clip.expect("clip")), Some(item));
    }
}
