/// Inline formatting context layout.
///
/// Handles `display: InlineBlock` items in a horizontal cursor with line wrap.
/// Items advance left-to-right until `cursor_x + item.margin_box_width > avail_w`,
/// at which point a new line box starts. Each line's height is the tallest
/// `margin_box_height` on that line.
///
/// Inline (text) items are not yet wired — only inline-block atomic units flow
/// here today. Mixing inline-block and block items in the same container falls
/// through to block layout (handled by the dispatcher in `mod.rs`).
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::{IntentValue, SignalEmitter, World};

use super::measure::measure_items;

/// Run an inline formatting context layout pass for `layout_id`.
///
/// Each TC child is treated as an atomic inline-block: its measured
/// margin box becomes the cursor advance. Wrap occurs when the cursor
/// would exceed `available_width`.
pub fn layout(world: &mut World, emit: &mut dyn SignalEmitter, layout_id: ComponentId) {
    let (items, avail_w_gu, _avail_h_gu, unit_scale) = measure_items(world, layout_id);

    let mut cursor_x_gu: f32 = 0.0;
    let mut cursor_y_gu: f32 = 0.0;
    let mut line_height_gu: f32 = 0.0;

    for item in &items {
        // Wrap to a new line if this item won't fit and we're not at the line start.
        if cursor_x_gu > 0.0 && cursor_x_gu + item.margin_box_width_gu > avail_w_gu {
            cursor_y_gu += line_height_gu;
            cursor_x_gu = 0.0;
            line_height_gu = 0.0;
        }

        let content_origin_x_gu = cursor_x_gu + item.margin_left_gu + item.padding_left_gu;
        let content_origin_y_gu = cursor_y_gu + item.margin_top_gu + item.padding_top_gu;

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

        cursor_x_gu += item.margin_box_width_gu;
        if item.margin_box_height_gu > line_height_gu {
            line_height_gu = item.margin_box_height_gu;
        }
    }
}
