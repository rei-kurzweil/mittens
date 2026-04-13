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
    StyleComponent, TransformComponent,
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
            .map(|s| (s.background_color, s.background_z, s.overflow))
    });

    let existing_bg = children.iter()
        .find(|&&ch| world.component_label(ch) == Some("__bg"))
        .copied();

    let needs_clip = bg_style
        .map(|(_, _, ov)| matches!(ov, Overflow::Hidden | Overflow::Scroll))
        .unwrap_or(false);

    match (bg_style, existing_bg) {
        // background_color present — ensure __bg exists and position it.
        (Some((Some(rgba), bg_z, _)), existing) => {
            let bg_id = match existing {
                Some(id) => id,
                None => spawn_bg_quad(world, emit, tc_id, rgba),
            };

            // The quad mesh is centered at its local origin (extends ±0.5 when scale=1).
            // Glyph quads are also centered at their column positions, so the visual
            // top-left of the text is at (−0.5, +0.5) in item TC local space, not at
            // the content origin (0, 0). The background must be shifted by (−0.5, +0.5)
            // to align its edges with the text's visual extent.
            //
            // Center of background in item TC local space (Y-up, glyph units):
            //   cx = box_width/2 − padding_left − 0.5
            //   cy = padding_top − box_height/2 + 0.5
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

            sync_stencil_clip(world, emit, bg_id, needs_clip);
        }

        // overflow: Hidden/Scroll with no background_color — still need a clip quad.
        (Some((None, bg_z, _)), existing) if needs_clip => {
            let bg_id = match existing {
                Some(id) => id,
                // Spawn with transparent color so geometry exists for the stencil write.
                None => spawn_bg_quad(world, emit, tc_id, [0.0, 0.0, 0.0, 0.0]),
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
            sync_stencil_clip(world, emit, bg_id, true);
        }

        // background_color cleared and no clip need — remove the stale __bg quad.
        (Some((None, _, _)) | None, Some(bg_id)) => {
            emit.push_intent_now(
                bg_id,
                IntentValue::RemoveSubtree { component_ids: vec![bg_id] },
            );
        }

        // No background_color, no __bg — nothing to do.
        _ => {}
    }
}

/// Attach or detach `StencilClipComponent` on `__bg_id` based on `needs_clip`.
fn sync_stencil_clip(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    bg_id: ComponentId,
    needs_clip: bool,
) {
    let clip_children: Vec<ComponentId> = world
        .children_of(bg_id)
        .iter()
        .copied()
        .filter(|&ch| world.get_component_by_id_as::<StencilClipComponent>(ch).is_some())
        .collect();

    let has_clip = !clip_children.is_empty();

    if needs_clip && !has_clip {
        let clip_id = world.add_component(StencilClipComponent::new());
        let _ = world.add_child(bg_id, clip_id);
        world.init_component_tree(clip_id, emit);
    } else if !needs_clip && has_clip {
        for clip_id in clip_children {
            emit.push_intent_now(
                clip_id,
                IntentValue::RemoveSubtree { component_ids: vec![clip_id] },
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
