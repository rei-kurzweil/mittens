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
/// each item TC and sized to cover the full padding box. The item TC must have
/// `scale ≈ TEXT_SCALE` so that glyph-unit positions in its local space equal
/// approximately one character cell in world space.
use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::{IntentValue, SignalEmitter};
use crate::engine::ecs::component::{
    ColorComponent, OpacityComponent, Overflow, RenderableComponent, StencilClipComponent,
    RaycastableComponent, RaycastableShapeComponent, RaycastableShapeType,
    StyleComponent, TransformComponent,
};
use super::measure::{measure_container_items, measure_items, MeasuredItem};

const OWNED_LAYOUT_STENCIL_CLIP_LABEL: &str = "__layout_stencil_clip";
const OWNED_SCROLL_DRAG_RAYCASTABLE_LABEL: &str = "__scroll_drag_raycastable";
const OWNED_SCROLL_DRAG_SHAPE_LABEL: &str = "__scroll_drag_shape";

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

        // ── Background quad ───────────────────────────────────────────────
        sync_bg_quad(world, emit, item.tc_id, item.padding_left_gu, item.padding_top_gu, item.box_width_gu, item.box_height_gu);

        let nested_items = measure_container_items(
            world,
            item.tc_id,
            item.content_width_gu,
            Some(item.content_height_gu),
        );
        if !nested_items.is_empty() {
            layout_items(world, emit, &nested_items, unit_scale);
        }

        cursor_gu += item.box_height_gu + item.margin_bottom_gu;
    }
}

/// Create, update, or remove the `__bg` child TC for a layout item.
///
/// The background quad covers the full padding box (content + padding on all sides).
/// Positions are in the item TC's local space (glyph units, since item TC scale ≈ TEXT_SCALE).
fn sync_bg_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
    padding_left_gu: f32,
    padding_top_gu: f32,
    box_width_gu: f32,
    box_height_gu: f32,
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
                        box_width_gu / 2.0 - padding_left_gu - 0.5,
                        padding_top_gu - box_height_gu / 2.0 + 0.5,
                        bg_z,
                    ],
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [box_width_gu, box_height_gu, 1.0],
                },
            );

            sync_stencil_clip(world, emit, tc_id, needs_clip);
            sync_scroll_drag_surface(world, emit, bg_id, needs_scroll_drag_surface);
            return;
        }
    }

    sync_stencil_clip(world, emit, tc_id, false);
    if let Some(bg_id) = existing_bg {
        sync_scroll_drag_surface(world, emit, bg_id, false);
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
