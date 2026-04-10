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
    ColorComponent, OpacityComponent, RenderableComponent, StyleComponent, TransformComponent,
};
use super::measure::measure_items;

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

    let mut cursor_gu = 0.0_f32;

    for item in &items {
        cursor_gu += item.margin_top_gu;

        let content_origin_y_gu = cursor_gu + item.padding_top_gu;
        let content_origin_x_gu = item.margin_left_gu + item.padding_left_gu;

        // Preserve the TC's existing scale — LayoutSystem controls position only.
        let tc_scale = world
            .get_component_by_id_as::<TransformComponent>(item.tc_id)
            .map(|tc| tc.transform.scale)
            .unwrap_or([1.0, 1.0, 1.0]);

        emit.push_intent_now(
            item.tc_id,
            IntentValue::UpdateTransform {
                component_ids: vec![item.tc_id],
                translation: [
                      content_origin_x_gu * unit_scale,
                    -(content_origin_y_gu * unit_scale),
                    0.0,
                ],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: tc_scale,
            },
        );

        // ── Background quad ───────────────────────────────────────────────
        sync_bg_quad(world, emit, item.tc_id, item.padding_left_gu, item.padding_top_gu, item.box_width_gu, item.box_height_gu);

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
            .map(|s| (s.background_color, s.background_z))
    });

    let existing_bg = children.iter()
        .find(|&&ch| world.component_label(ch) == Some("__bg"))
        .copied();

    match (bg_style, existing_bg) {
        // background_color present — ensure __bg exists and position it.
        (Some((Some(rgba), bg_z)), existing) => {
            let bg_id = match existing {
                Some(id) => id,
                None => spawn_bg_quad(world, emit, tc_id, rgba),
            };

            emit.push_intent_now(
                bg_id,
                IntentValue::UpdateTransform {
                    component_ids: vec![bg_id],
                    // Offset back from content origin to top-left of padding box.
                    translation: [-padding_left_gu, padding_top_gu, bg_z],
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [box_width_gu, box_height_gu, 1.0],
                },
            );
        }

        // background_color cleared — remove the stale __bg quad.
        (Some((None, _)) | None, Some(bg_id)) => {
            emit.push_intent_now(
                bg_id,
                IntentValue::RemoveSubtree { component_ids: vec![bg_id] },
            );
        }

        // No background_color, no __bg — nothing to do.
        _ => {}
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
