use crate::engine::ecs::component::TransformComponent;
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
use crate::engine::ecs::{IntentValue, SignalEmitter, World};

use super::block::apply_text_align;
use super::box_model_viz::sync_box_model_viz;
use super::measure::{
    apply_text_color_for_item, apply_text_font_size_for_item, apply_text_wrap_for_item,
    measure_container_items, measure_items, MeasuredItem,
};
use crate::engine::ecs::component::style::Display;

/// Run an inline formatting context layout pass for `layout_id`.
///
/// Each TC child is treated as an atomic inline-block: its measured
/// margin box becomes the cursor advance. Wrap occurs when the cursor
/// would exceed `available_width`.
pub fn layout(world: &mut World, emit: &mut dyn SignalEmitter, layout_id: ComponentId) {
    let (items, avail_w_gu, _avail_h_gu, unit_scale) = measure_items(world, layout_id);
    let viz = super::block::layout_root_has_inspect(world, layout_id);
    let axis_scales = super::measure::layout_root_axis_scales(world, layout_id);
    layout_items(
        world,
        emit,
        &items,
        avail_w_gu,
        unit_scale,
        axis_scales,
        0,
        0,
        viz,
    );
}

/// Inline-formatting-context layout over a pre-measured item list.
///
/// `avail_w_gu` is the inline-axis budget (in glyph units) the parent
/// passes down — for the LayoutRoot case that's `LayoutComponent.available_width`;
/// for a nested block item that switches to inline flow, it's
/// `item.content_width_gu` of the enclosing block.
pub(crate) fn layout_items(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    items: &[MeasuredItem],
    avail_w_gu: f32,
    unit_scale: f32,
    axis_scales: (f32, f32),
    depth: i32,
    parent_depth: i32,
    viz: bool,
) {
    let mut cursor_x_gu: f32 = 0.0;
    let mut cursor_y_gu: f32 = 0.0;
    let mut line_height_gu: f32 = 0.0;
    let resolved_z = (depth - parent_depth) as f32 * super::LAYER_DISTANCE;

    for original in items {
        // Auto-width inline-block items consume the remaining inline-axis budget
        // on this line — re-measure with that as their available width so the
        // wrap test below sees the actual width, and intrinsic height
        // (text wrap, child layout) is computed at the final width.
        let item: MeasuredItem = if original.is_auto_width {
            let remaining = (avail_w_gu - cursor_x_gu).max(0.0);
            super::measure::measure_item(
                world,
                original.tc_id,
                remaining,
                Some(original.content_height_gu),
                unit_scale,
            )
        } else {
            original.clone()
        };
        let item = &item;

        // Wrap to a new line if this item won't fit and we're not at the line start.
        if cursor_x_gu > 0.0 && cursor_x_gu + item.margin_box_width_gu > avail_w_gu {
            cursor_y_gu += line_height_gu;
            cursor_x_gu = 0.0;
            line_height_gu = 0.0;
        }

        let content_origin_x_gu = cursor_x_gu + item.margin_left_gu + item.padding_left_gu;
        let content_origin_y_gu = cursor_y_gu + item.margin_top_gu + item.padding_top_gu;

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

        if super::measure::trace_layout_id(world, item.tc_id) {
            println!(
                "[layout-trace] place-inline item={} id={:?} cursor_gu=({:.6},{:.6}) content_origin_gu=({:.6},{:.6}) local_translation=({:.6},{:.6},{:.6}) final_delta_wu=({:.6},{:.6}) item_box_final_wu=({:.6},{:.6})",
                super::measure::trace_label(world, item.tc_id),
                item.tc_id,
                cursor_x_gu,
                cursor_y_gu,
                content_origin_x_gu,
                content_origin_y_gu,
                translation[0],
                translation[1],
                translation[2],
                translation[0] * axis_scales.0,
                translation[1] * axis_scales.1,
                item.box_width_gu * unit_scale * axis_scales.0,
                item.box_height_gu * unit_scale * axis_scales.1,
            );
        }

        emit.push_intent_now(
            item.tc_id,
            IntentValue::UpdateTransform {
                component_ids: vec![item.tc_id],
                translation,
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: tc_scale,
            },
        );

        apply_text_font_size_for_item(world, emit, item.tc_id, unit_scale);
        apply_text_wrap_for_item(world, emit, item.tc_id, item.content_width_gu, unit_scale);
        apply_text_color_for_item(world, emit, item.tc_id);

        // Background quad — share the block-flow implementation so
        // `Style { background_color }` works consistently for both
        // formatting contexts.
        super::block::sync_bg_quad(
            world,
            emit,
            item.tc_id,
            item.padding_left_gu,
            item.padding_top_gu,
            item.box_width_gu,
            item.box_height_gu,
            unit_scale,
        );
        super::block::sync_auto_text_lift(world, emit, item.tc_id);
        sync_box_model_viz(world, emit, item, unit_scale, viz);
        apply_text_align(
            world,
            emit,
            item.tc_id,
            item.content_width_gu,
            item.content_height_gu,
            unit_scale,
        );
        let content_root =
            super::block::sync_overflow_topology(world, emit, item.tc_id, item.content_height_gu);

        // Recurse into the item's own children using whichever formatting
        // context their `display` modes call for. Inline-block items can
        // host either inline children (more text/icons) or block children
        // (a stacked sub-tree); both must be honored.
        let nested_items =
            measure_container_items(world, content_root, item.content_width_gu, None, unit_scale);
        if !nested_items.is_empty() {
            let all_inline_block = nested_items
                .iter()
                .all(|it| matches!(it.display, Some(Display::InlineBlock | Display::Inline)));
            let child_depth = if super::block::item_owns_layer(world, item.tc_id) {
                depth + 1
            } else {
                depth
            };
            if all_inline_block {
                layout_items(
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
                super::block::layout_items_for(
                    world,
                    emit,
                    &nested_items,
                    unit_scale,
                    child_depth,
                    depth,
                    viz,
                );
            }
        }

        cursor_x_gu += item.margin_box_width_gu;
        if item.margin_box_height_gu > line_height_gu {
            line_height_gu = item.margin_box_height_gu;
        }
    }
}
