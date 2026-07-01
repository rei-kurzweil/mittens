use crate::engine::ecs::ComponentId;
use crate::engine::ecs::IntentValue;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::World;
use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::component::style::{
    AlignItems, FlexDirection, JustifyContent, SizeDimension,
};
use crate::engine::ecs::component::{HtmlElementComponent, StyleComponent};

use super::block::{
    apply_text_align, item_owns_layer, layout_root_has_inspect, sync_auto_text_lift, sync_bg_quad,
    sync_layout_bounds, sync_overflow_topology, sync_scrolling_metrics,
};
use super::box_model_viz::sync_box_model_viz;
use super::measure::{
    MeasuredItem, apply_text_color_for_item, apply_text_font_size_for_item,
    apply_text_wrap_for_item, measure_container_items, measure_item, measure_items,
};

#[derive(Debug, Clone, Copy)]
struct FlexContainerStyle {
    direction: FlexDirection,
    justify_content: JustifyContent,
    align_items: AlignItems,
    row_gap: f32,
    column_gap: f32,
}

#[derive(Debug, Clone, Copy)]
struct FlexItemStyle {
    flex_grow: f32,
    width: SizeDimension,
    height: SizeDimension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
struct FlexItem {
    measured: MeasuredItem,
    style: FlexItemStyle,
}

/// Run a flex formatting context layout pass for `layout_id`.
pub fn layout(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    layout_id: ComponentId,
) -> (f32, f32) {
    let (items, avail_w_gu, avail_h_gu, unit_scale) = measure_items(world, layout_id);
    let viz = layout_root_has_inspect(world, layout_id);
    let axis_scales = super::measure::layout_root_axis_scales(world, layout_id);
    layout_items(
        world,
        emit,
        layout_id,
        &items,
        avail_w_gu,
        avail_h_gu,
        unit_scale,
        axis_scales,
        0,
        0,
        viz,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_items(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    container_id: ComponentId,
    items: &[MeasuredItem],
    avail_w_gu: f32,
    avail_h_gu: Option<f32>,
    unit_scale: f32,
    axis_scales: (f32, f32),
    depth: i32,
    parent_depth: i32,
    viz: bool,
) -> (f32, f32) {
    let container_style = container_style(world, container_id);
    let main_axis = match container_style.direction {
        FlexDirection::Column | FlexDirection::ColumnReverse => MainAxis::Vertical,
        FlexDirection::Row | FlexDirection::RowReverse => MainAxis::Horizontal,
    };
    let gap_gu = match main_axis {
        MainAxis::Horizontal => container_style.column_gap,
        MainAxis::Vertical => container_style.row_gap,
    };

    let mut flex_items: Vec<FlexItem> = items
        .iter()
        .cloned()
        .map(|measured| {
            let style = item_style(world, measured.tc_id);
            FlexItem { measured, style }
        })
        .collect();

    let container_main_gu = match main_axis {
        MainAxis::Horizontal => Some(avail_w_gu),
        MainAxis::Vertical => avail_h_gu,
    };
    let container_cross_gu = match main_axis {
        MainAxis::Horizontal => avail_h_gu,
        MainAxis::Vertical => Some(avail_w_gu),
    };

    let gap_total = gap_gu * flex_items.len().saturating_sub(1) as f32;
    let base_main_total = flex_items
        .iter()
        .map(|item| main_margin_box_size(&item.measured, main_axis))
        .sum::<f32>();
    let total_grow = flex_items
        .iter()
        .map(|item| item.style.flex_grow.max(0.0))
        .sum::<f32>();

    if let Some(main_size) = container_main_gu {
        let free_space = (main_size - base_main_total - gap_total).max(0.0);
        if free_space > 0.0 && total_grow > 0.0 {
            for item in &mut flex_items {
                if item.style.flex_grow <= 0.0 {
                    continue;
                }
                let grown_margin_box = main_margin_box_size(&item.measured, main_axis)
                    + free_space * item.style.flex_grow / total_grow;
                item.measured = remeasure_main_axis(
                    world,
                    item.measured.clone(),
                    item.style,
                    main_axis,
                    grown_margin_box,
                    avail_w_gu,
                    avail_h_gu,
                    unit_scale,
                );
            }
        }
    }

    if matches!(container_style.align_items, AlignItems::Stretch) {
        if let Some(cross_size) = container_cross_gu {
            for item in &mut flex_items {
                if !cross_axis_is_auto(item.style, main_axis) {
                    continue;
                }
                let stretched_margin_box =
                    (cross_size - cross_margin_sum(&item.measured, main_axis)).max(0.0);
                item.measured = remeasure_cross_axis(
                    world,
                    item.measured.clone(),
                    item.style,
                    main_axis,
                    stretched_margin_box,
                    avail_w_gu,
                    avail_h_gu,
                    unit_scale,
                );
            }
        }
    }

    let used_main_gu = flex_items
        .iter()
        .map(|item| main_margin_box_size(&item.measured, main_axis))
        .sum::<f32>()
        + gap_total;
    let content_cross_gu = flex_items
        .iter()
        .map(|item| cross_margin_box_size(&item.measured, main_axis))
        .fold(0.0, f32::max);
    let layout_cross_gu = container_cross_gu.unwrap_or(content_cross_gu);
    let layout_main_gu = container_main_gu.unwrap_or(used_main_gu).max(used_main_gu);
    let leading_main_offset = justify_offset(
        container_style.justify_content,
        layout_main_gu,
        used_main_gu,
        flex_items.len(),
    );
    let between_gap = justify_gap(
        container_style.justify_content,
        gap_gu,
        layout_main_gu,
        used_main_gu,
        flex_items.len(),
    );
    let resolved_z = (depth - parent_depth) as f32 * super::LAYER_DISTANCE;

    let mut cursor_main_gu = leading_main_offset;
    for item in &flex_items {
        let cross_offset_gu = align_cross_offset(
            container_style.align_items,
            layout_cross_gu,
            cross_margin_box_size(&item.measured, main_axis),
        );

        let (content_origin_x_gu, content_origin_y_gu) = match main_axis {
            MainAxis::Horizontal => (
                cursor_main_gu + item.measured.margin_left_gu + item.measured.padding_left_gu,
                cross_offset_gu + item.measured.margin_top_gu + item.measured.padding_top_gu,
            ),
            MainAxis::Vertical => (
                cross_offset_gu + item.measured.margin_left_gu + item.measured.padding_left_gu,
                cursor_main_gu + item.measured.margin_top_gu + item.measured.padding_top_gu,
            ),
        };

        let tc_scale = world
            .get_component_by_id_as::<TransformComponent>(item.measured.tc_id)
            .map(|tc| tc.transform.scale)
            .unwrap_or([1.0, 1.0, 1.0]);
        emit.push_intent_now(
            item.measured.tc_id,
            IntentValue::UpdateTransform {
                component_ids: vec![item.measured.tc_id],
                translation: [
                    content_origin_x_gu * unit_scale,
                    -(content_origin_y_gu * unit_scale),
                    resolved_z,
                ],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: tc_scale,
            },
        );

        sync_layout_bounds(world, emit, &item.measured, unit_scale);
        apply_text_font_size_for_item(world, emit, item.measured.tc_id, unit_scale);
        apply_text_wrap_for_item(
            world,
            emit,
            item.measured.tc_id,
            item.measured.content_width_gu,
            unit_scale,
        );
        apply_text_color_for_item(world, emit, item.measured.tc_id);
        sync_bg_quad(
            world,
            emit,
            item.measured.tc_id,
            item.measured.padding_left_gu,
            item.measured.padding_top_gu,
            item.measured.box_width_gu,
            item.measured.box_height_gu,
            unit_scale,
        );
        sync_auto_text_lift(world, emit, item.measured.tc_id);
        sync_box_model_viz(world, emit, &item.measured, unit_scale, viz);
        apply_text_align(
            world,
            emit,
            item.measured.tc_id,
            item.measured.content_width_gu,
            item.measured.content_height_gu,
            unit_scale,
        );
        let content_root = sync_overflow_topology(
            world,
            emit,
            item.measured.tc_id,
            item.measured.content_height_gu,
        );
        let nested_items = measure_container_items(
            world,
            content_root,
            item.measured.content_width_gu,
            Some(item.measured.content_height_gu),
            unit_scale,
        );
        if let Some(scroll_id) = immediate_owned_scroll_wrapper(world, item.measured.tc_id) {
            sync_scrolling_metrics(
                world,
                emit,
                scroll_id,
                item.measured.content_height_gu,
                &nested_items,
            );
        }
        if !nested_items.is_empty() {
            let child_depth = if item_owns_layer(world, item.measured.tc_id) {
                depth + 1
            } else {
                depth
            };
            super::layout_container_items(
                world,
                emit,
                item.measured.tc_id,
                &nested_items,
                item.measured.content_width_gu,
                Some(item.measured.content_height_gu),
                unit_scale,
                axis_scales,
                child_depth,
                depth,
                viz,
            );
        }

        cursor_main_gu += main_margin_box_size(&item.measured, main_axis) + between_gap;
    }

    match main_axis {
        MainAxis::Horizontal => (layout_main_gu, layout_cross_gu),
        MainAxis::Vertical => (layout_cross_gu, layout_main_gu),
    }
}

fn container_style(world: &World, container_id: ComponentId) -> FlexContainerStyle {
    let children = world.children_of(container_id);
    if let Some(style) = children
        .iter()
        .find_map(|&child| world.get_component_by_id_as::<StyleComponent>(child))
    {
        return FlexContainerStyle {
            direction: style.flex_direction,
            justify_content: style.justify_content,
            align_items: style.align_items,
            row_gap: style.row_gap,
            column_gap: style.column_gap,
        };
    }

    let _ua_display = children.iter().find_map(|&child| {
        world
            .get_component_by_id_as::<HtmlElementComponent>(child)
            .and_then(|el| el.element_type.default_display())
    });

    FlexContainerStyle {
        direction: FlexDirection::Row,
        justify_content: JustifyContent::FlexStart,
        align_items: AlignItems::Stretch,
        row_gap: 0.0,
        column_gap: 0.0,
    }
}

fn item_style(world: &World, tc_id: ComponentId) -> FlexItemStyle {
    let children = world.children_of(tc_id);
    if let Some(style) = children
        .iter()
        .find_map(|&child| world.get_component_by_id_as::<StyleComponent>(child))
    {
        return FlexItemStyle {
            flex_grow: style.flex_grow,
            width: style.width,
            height: style.height,
        };
    }

    FlexItemStyle {
        flex_grow: 0.0,
        width: SizeDimension::Auto,
        height: SizeDimension::Auto,
    }
}

fn main_margin_box_size(item: &MeasuredItem, main_axis: MainAxis) -> f32 {
    match main_axis {
        MainAxis::Horizontal => item.margin_box_width_gu,
        MainAxis::Vertical => item.margin_box_height_gu,
    }
}

fn cross_margin_box_size(item: &MeasuredItem, main_axis: MainAxis) -> f32 {
    match main_axis {
        MainAxis::Horizontal => item.margin_box_height_gu,
        MainAxis::Vertical => item.margin_box_width_gu,
    }
}

fn cross_margin_sum(item: &MeasuredItem, main_axis: MainAxis) -> f32 {
    match main_axis {
        MainAxis::Horizontal => item.margin_top_gu + item.margin_bottom_gu,
        MainAxis::Vertical => item.margin_left_gu + item.margin_right_gu,
    }
}

fn cross_axis_is_auto(style: FlexItemStyle, main_axis: MainAxis) -> bool {
    match main_axis {
        MainAxis::Horizontal => matches!(style.height, SizeDimension::Auto),
        MainAxis::Vertical => matches!(style.width, SizeDimension::Auto),
    }
}

fn remeasure_main_axis(
    world: &World,
    current: MeasuredItem,
    style: FlexItemStyle,
    main_axis: MainAxis,
    target_margin_box_gu: f32,
    avail_w_gu: f32,
    avail_h_gu: Option<f32>,
    unit_scale: f32,
) -> MeasuredItem {
    let mut measured = match main_axis {
        MainAxis::Horizontal => measure_item(
            world,
            current.tc_id,
            target_margin_box_gu.max(0.0),
            avail_h_gu,
            unit_scale,
        ),
        MainAxis::Vertical => current.clone(),
    };
    let current_main = main_margin_box_size(&measured, main_axis);
    if target_margin_box_gu > current_main {
        grow_main_axis(
            &mut measured,
            main_axis,
            target_margin_box_gu - current_main,
        );
    }
    let _ = (style, avail_w_gu);
    measured
}

fn remeasure_cross_axis(
    world: &World,
    current: MeasuredItem,
    _style: FlexItemStyle,
    main_axis: MainAxis,
    target_margin_box_gu: f32,
    avail_w_gu: f32,
    avail_h_gu: Option<f32>,
    unit_scale: f32,
) -> MeasuredItem {
    let mut measured = match main_axis {
        MainAxis::Horizontal => current.clone(),
        MainAxis::Vertical => measure_item(
            world,
            current.tc_id,
            target_margin_box_gu.max(0.0),
            avail_h_gu,
            unit_scale,
        ),
    };
    let current_cross = cross_margin_box_size(&measured, main_axis);
    if target_margin_box_gu > current_cross {
        grow_cross_axis(
            &mut measured,
            main_axis,
            target_margin_box_gu - current_cross,
        );
    }
    if matches!(main_axis, MainAxis::Horizontal) {
        let _ = avail_w_gu;
    }
    measured
}

fn grow_main_axis(item: &mut MeasuredItem, main_axis: MainAxis, delta: f32) {
    if delta <= 0.0 {
        return;
    }
    match main_axis {
        MainAxis::Horizontal => {
            item.content_width_gu += delta;
            item.box_width_gu += delta;
            item.margin_box_width_gu += delta;
        }
        MainAxis::Vertical => {
            item.content_height_gu += delta;
            item.box_height_gu += delta;
            item.margin_box_height_gu += delta;
        }
    }
}

fn grow_cross_axis(item: &mut MeasuredItem, main_axis: MainAxis, delta: f32) {
    if delta <= 0.0 {
        return;
    }
    match main_axis {
        MainAxis::Horizontal => {
            item.content_height_gu += delta;
            item.box_height_gu += delta;
            item.margin_box_height_gu += delta;
        }
        MainAxis::Vertical => {
            item.content_width_gu += delta;
            item.box_width_gu += delta;
            item.margin_box_width_gu += delta;
        }
    }
}

fn justify_offset(
    justify: JustifyContent,
    container_main_gu: f32,
    used_main_gu: f32,
    item_count: usize,
) -> f32 {
    let free = (container_main_gu - used_main_gu).max(0.0);
    match justify {
        JustifyContent::Center => free * 0.5,
        JustifyContent::FlexEnd => free,
        JustifyContent::SpaceBetween if item_count <= 1 => free * 0.5,
        _ => 0.0,
    }
}

fn justify_gap(
    justify: JustifyContent,
    base_gap_gu: f32,
    container_main_gu: f32,
    used_main_gu: f32,
    item_count: usize,
) -> f32 {
    if !matches!(justify, JustifyContent::SpaceBetween) || item_count <= 1 {
        return base_gap_gu;
    }
    let free = (container_main_gu - used_main_gu).max(0.0);
    base_gap_gu + free / (item_count - 1) as f32
}

fn align_cross_offset(align: AlignItems, container_cross_gu: f32, item_cross_gu: f32) -> f32 {
    let free = (container_cross_gu - item_cross_gu).max(0.0);
    match align {
        AlignItems::Center => free * 0.5,
        AlignItems::FlexEnd => free,
        _ => 0.0,
    }
}

fn immediate_owned_scroll_wrapper(world: &World, tc_id: ComponentId) -> Option<ComponentId> {
    world.children_of(tc_id).iter().copied().find(|&child| {
        world.component_label(child) == Some("__scroll")
            && world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::LayoutComponent;
    use crate::engine::ecs::rx::{EventSignal, IntentValue};
    use crate::engine::ecs::system::layout::LayoutSystem;
    use crate::engine::ecs::{IntentSignal, SignalEmitter};

    #[derive(Default)]
    struct TestEmitter {
        intents: Vec<(ComponentId, IntentSignal)>,
    }

    impl SignalEmitter for TestEmitter {
        fn push_event(&mut self, _: ComponentId, _: EventSignal) {}

        fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
            self.intents.push((scope, intent));
        }
    }

    fn item_tc(world: &mut World, name: &'static str, width: f32, height: f32) -> ComponentId {
        let tc = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
        let style = world.add_component_boxed_named(
            "style",
            Box::new({
                let mut s = StyleComponent::new();
                s.width = SizeDimension::GlyphUnits(width);
                s.height = SizeDimension::GlyphUnits(height);
                s
            }),
        );
        let _ = world.add_child(tc, style);
        tc
    }

    fn layout_translation(emitter: &TestEmitter, tc_id: ComponentId) -> [f32; 3] {
        emitter
            .intents
            .iter()
            .find_map(|(_, intent)| match &intent.value {
                IntentValue::UpdateTransform {
                    component_ids,
                    translation,
                    ..
                } if component_ids == &vec![tc_id] => Some(*translation),
                _ => None,
            })
            .unwrap()
    }

    fn flex_root(world: &mut World, width: f32, height: Option<f32>) -> ComponentId {
        let root = match height {
            Some(h) => world.add_component(LayoutComponent::new(width).with_height(h)),
            None => world.add_component(LayoutComponent::new(width)),
        };
        let style = world.add_component_boxed_named(
            "root_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(crate::engine::ecs::component::style::Display::Flex);
                s
            }),
        );
        let _ = world.add_child(root, style);
        root
    }

    fn set_style<F>(world: &mut World, tc_id: ComponentId, f: F)
    where
        F: FnOnce(&mut StyleComponent),
    {
        let style_id = world
            .children_of(tc_id)
            .iter()
            .copied()
            .find(|&id| world.get_component_by_id_as::<StyleComponent>(id).is_some())
            .unwrap();
        let style = world
            .get_component_by_id_as_mut::<StyleComponent>(style_id)
            .unwrap();
        f(style);
    }

    #[test]
    fn flex_row_places_children_horizontally_in_order() {
        let mut world = World::default();
        let root = flex_root(&mut world, 30.0, None);
        let a = item_tc(&mut world, "a", 3.0, 2.0);
        let b = item_tc(&mut world, "b", 4.0, 2.0);
        let c = item_tc(&mut world, "c", 5.0, 2.0);
        let _ = world.add_child(root, a);
        let _ = world.add_child(root, b);
        let _ = world.add_child(root, c);

        let mut emit = TestEmitter::default();
        LayoutSystem::new().tick(&mut world, &mut emit);

        assert_eq!(layout_translation(&emit, a), [0.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, b), [3.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, c), [7.0, 0.0, 0.0]);
    }

    #[test]
    fn flex_column_places_children_vertically() {
        let mut world = World::default();
        let root = flex_root(&mut world, 20.0, None);
        set_style(&mut world, root, |style| {
            style.flex_direction = FlexDirection::Column;
        });
        let a = item_tc(&mut world, "a", 3.0, 2.0);
        let b = item_tc(&mut world, "b", 4.0, 3.0);
        let c = item_tc(&mut world, "c", 5.0, 4.0);
        let _ = world.add_child(root, a);
        let _ = world.add_child(root, b);
        let _ = world.add_child(root, c);

        let mut emit = TestEmitter::default();
        LayoutSystem::new().tick(&mut world, &mut emit);

        assert_eq!(layout_translation(&emit, a), [0.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, b), [0.0, -2.0, 0.0]);
        assert_eq!(layout_translation(&emit, c), [0.0, -5.0, 0.0]);
    }

    #[test]
    fn flex_gap_and_grow_and_alignment_apply() {
        let mut world = World::default();
        let root = flex_root(&mut world, 20.0, Some(10.0));
        set_style(&mut world, root, |style| {
            style.column_gap = 2.0;
            style.justify_content = JustifyContent::Center;
            style.align_items = AlignItems::Center;
        });
        let a = item_tc(&mut world, "a", 4.0, 2.0);
        let b = item_tc(&mut world, "b", 4.0, 4.0);
        let c = item_tc(&mut world, "c", 4.0, 2.0);
        for &child in &[a, b, c] {
            let _ = world.add_child(root, child);
        }
        set_style(&mut world, b, |style| {
            style.flex_grow = 1.0;
        });

        let mut emit = TestEmitter::default();
        LayoutSystem::new().tick(&mut world, &mut emit);

        assert_eq!(layout_translation(&emit, a), [0.0, -4.0, 0.0]);
        assert_eq!(layout_translation(&emit, b), [6.0, -3.0, 0.0]);
        assert_eq!(layout_translation(&emit, c), [16.0, -4.0, 0.0]);
    }

    #[test]
    fn flex_space_between_distributes_remaining_width() {
        let mut world = World::default();
        let root = flex_root(&mut world, 20.0, None);
        set_style(&mut world, root, |style| {
            style.justify_content = JustifyContent::SpaceBetween;
        });
        let a = item_tc(&mut world, "a", 2.0, 2.0);
        let b = item_tc(&mut world, "b", 2.0, 2.0);
        let c = item_tc(&mut world, "c", 2.0, 2.0);
        let _ = world.add_child(root, a);
        let _ = world.add_child(root, b);
        let _ = world.add_child(root, c);

        let mut emit = TestEmitter::default();
        LayoutSystem::new().tick(&mut world, &mut emit);

        assert_eq!(layout_translation(&emit, a), [0.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, b), [9.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, c), [18.0, 0.0, 0.0]);
    }

    #[test]
    fn nested_flex_dispatches_inside_block_and_flex_recurses_to_block_children() {
        let mut world = World::default();
        let root = world.add_component(LayoutComponent::new(30.0));
        let panel = item_tc(&mut world, "panel", 30.0, 10.0);
        let flex = world.add_component_boxed_named("flex", Box::new(TransformComponent::new()));
        let flex_style = world.add_component_boxed_named(
            "flex_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.display = Some(crate::engine::ecs::component::style::Display::Flex);
                s.width = SizeDimension::GlyphUnits(30.0);
                s.height = SizeDimension::GlyphUnits(10.0);
                s.column_gap = 1.0;
                s
            }),
        );
        let left = item_tc(&mut world, "left", 4.0, 2.0);
        let right = item_tc(&mut world, "right", 4.0, 2.0);
        let nested_block =
            world.add_component_boxed_named("nested_block", Box::new(TransformComponent::new()));
        let nested_block_style = world.add_component_boxed_named(
            "nested_block_style",
            Box::new({
                let mut s = StyleComponent::new();
                s.width = SizeDimension::GlyphUnits(4.0);
                s.height = SizeDimension::GlyphUnits(4.0);
                s
            }),
        );
        let inner = item_tc(&mut world, "inner", 4.0, 1.0);

        let _ = world.add_child(root, panel);
        let _ = world.add_child(panel, flex);
        let _ = world.add_child(flex, flex_style);
        let _ = world.add_child(flex, left);
        let _ = world.add_child(flex, right);
        let _ = world.add_child(right, nested_block);
        let _ = world.add_child(nested_block, nested_block_style);
        let _ = world.add_child(nested_block, inner);

        let mut emit = TestEmitter::default();
        LayoutSystem::new().tick(&mut world, &mut emit);

        assert_eq!(layout_translation(&emit, left), [0.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, right), [5.0, 0.0, 0.0]);
        assert_eq!(layout_translation(&emit, inner), [0.0, 0.0, 0.0]);
    }
}
