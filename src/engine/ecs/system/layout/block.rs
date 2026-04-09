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
use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::{IntentValue, SignalEmitter};
use super::measure::measure_items;

/// Run a block formatting context layout pass for `layout_id`.
///
/// Calls `measure_items` (Pass 1) then walks the results with a vertical cursor
/// and emits `UpdateTransform` for each TC child.
pub fn layout(
    world: &World,
    emit: &mut dyn SignalEmitter,
    layout_id: ComponentId,
) {
    let (items, _avail_w, _avail_h, unit_scale) = measure_items(world, layout_id);

    let mut cursor_gu = 0.0_f32;

    for item in &items {
        cursor_gu += item.margin_top_gu;

        let content_origin_y_gu = cursor_gu + item.padding_top_gu;
        let content_origin_x_gu = item.margin_left_gu + item.padding_left_gu;

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
                scale: [1.0, 1.0, 1.0],
            },
        );

        cursor_gu += item.box_height_gu + item.margin_bottom_gu;
    }
}
