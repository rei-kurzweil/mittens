use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::ecs::component::{ColorComponent, OpacityComponent, OverlayComponent, RenderableComponent, TransformComponent};

use super::measure::MeasuredItem;

const OWNED_BOX_MODEL_VIZ_ROOT: &str = "__box_model_viz";
const OWNED_BOX_MODEL_VIZ_OVERLAY: &str = "__box_model_viz_overlay";

const BOX_PADDING_TOP_LABEL: &str = "__box_padding_top";
const BOX_PADDING_RIGHT_LABEL: &str = "__box_padding_right";
const BOX_PADDING_BOTTOM_LABEL: &str = "__box_padding_bottom";
const BOX_PADDING_LEFT_LABEL: &str = "__box_padding_left";
const BOX_CONTENT_LABEL: &str = "__box_content";
const BOX_MARGIN_TOP_LABEL: &str = "__box_margin_top";
const BOX_MARGIN_RIGHT_LABEL: &str = "__box_margin_right";
const BOX_MARGIN_BOTTOM_LABEL: &str = "__box_margin_bottom";
const BOX_MARGIN_LEFT_LABEL: &str = "__box_margin_left";

const PADDING_RGBA: [f32; 4] = [0.0, 1.0, 1.0, 0.55];
const CONTENT_RGBA: [f32; 4] = [0.82, 0.82, 0.82, 0.22];
const MARGIN_RGBA: [f32; 4] = [1.0, 0.0, 0.65, 0.72];
const MARKER_RGBA: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

const MARKER_NARROW_GU: f32 = 0.18;

const Z_PADDING: f32 = 0.0;
const Z_CONTENT: f32 = 0.002;
const Z_MARGIN: f32 = 0.004;
const Z_MARKER: f32 = 0.006;

#[derive(Clone, Copy)]
enum MarkerOrientation {
    Vertical,
    Horizontal,
}

pub(crate) fn sync_box_model_viz(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    item: &MeasuredItem,
    unit_scale: f32,
    enabled: bool,
) {
    if !enabled {
        remove_box_model_viz(world, emit, item.tc_id);
        return;
    }

    let viz_root = ensure_viz_root(world, emit, item.tc_id);
    let overlay_root = ensure_overlay_root(world, emit, viz_root);

    sync_optional_padding_quad(
        world,
        emit,
        overlay_root,
        BOX_PADDING_TOP_LABEL,
        -item.padding_left_gu,
        -item.padding_top_gu,
        item.box_width_gu,
        item.padding_top_gu,
        MarkerOrientation::Vertical,
        unit_scale,
    );
    sync_optional_padding_quad(
        world,
        emit,
        overlay_root,
        BOX_PADDING_RIGHT_LABEL,
        item.content_width_gu,
        0.0,
        item.padding_right_gu,
        item.content_height_gu,
        MarkerOrientation::Horizontal,
        unit_scale,
    );
    sync_optional_padding_quad(
        world,
        emit,
        overlay_root,
        BOX_PADDING_BOTTOM_LABEL,
        -item.padding_left_gu,
        item.content_height_gu,
        item.box_width_gu,
        item.padding_bottom_gu,
        MarkerOrientation::Vertical,
        unit_scale,
    );
    sync_optional_padding_quad(
        world,
        emit,
        overlay_root,
        BOX_PADDING_LEFT_LABEL,
        -item.padding_left_gu,
        0.0,
        item.padding_left_gu,
        item.content_height_gu,
        MarkerOrientation::Horizontal,
        unit_scale,
    );

    sync_viz_quad(
        world,
        emit,
        overlay_root,
        BOX_CONTENT_LABEL,
        CONTENT_RGBA,
        0.0,
        0.0,
        item.content_width_gu,
        item.content_height_gu,
        Z_CONTENT,
        unit_scale,
    );

    sync_optional_margin_quad(
        world,
        emit,
        overlay_root,
        BOX_MARGIN_TOP_LABEL,
        -item.margin_left_gu - item.padding_left_gu,
        -item.margin_top_gu - item.padding_top_gu,
        item.margin_box_width_gu,
        item.margin_top_gu,
        MarkerOrientation::Vertical,
        unit_scale,
    );
    sync_optional_margin_quad(
        world,
        emit,
        overlay_root,
        BOX_MARGIN_RIGHT_LABEL,
        item.content_width_gu + item.padding_right_gu,
        -item.margin_top_gu - item.padding_top_gu,
        item.margin_right_gu,
        item.margin_box_height_gu,
        MarkerOrientation::Horizontal,
        unit_scale,
    );
    sync_optional_margin_quad(
        world,
        emit,
        overlay_root,
        BOX_MARGIN_BOTTOM_LABEL,
        -item.margin_left_gu - item.padding_left_gu,
        item.content_height_gu + item.padding_bottom_gu,
        item.margin_box_width_gu,
        item.margin_bottom_gu,
        MarkerOrientation::Vertical,
        unit_scale,
    );
    sync_optional_margin_quad(
        world,
        emit,
        overlay_root,
        BOX_MARGIN_LEFT_LABEL,
        -item.margin_left_gu - item.padding_left_gu,
        -item.margin_top_gu - item.padding_top_gu,
        item.margin_left_gu,
        item.margin_box_height_gu,
        MarkerOrientation::Horizontal,
        unit_scale,
    );
}

fn remove_box_model_viz(world: &mut World, emit: &mut dyn SignalEmitter, owner: ComponentId) {
    if let Some(viz_root) = immediate_owned_viz_root(world, owner) {
        emit.push_intent_now(
            viz_root,
            IntentValue::RemoveSubtree { component_ids: vec![viz_root] },
        );
    }
}

fn ensure_viz_root(world: &mut World, emit: &mut dyn SignalEmitter, owner: ComponentId) -> ComponentId {
    if let Some(viz_root) = immediate_owned_viz_root(world, owner) {
        return viz_root;
    }

    let viz_root = world.add_component_boxed_named(OWNED_BOX_MODEL_VIZ_ROOT, Box::new(TransformComponent::new()));
    let _ = world.add_child(owner, viz_root);
    world.init_component_tree(viz_root, emit);
    viz_root
}

fn ensure_overlay_root(world: &mut World, emit: &mut dyn SignalEmitter, viz_root: ComponentId) -> ComponentId {
    if let Some(overlay_root) = immediate_owned_overlay_root(world, viz_root) {
        return overlay_root;
    }

    let overlay_root = world.add_component_boxed_named(OWNED_BOX_MODEL_VIZ_OVERLAY, Box::new(OverlayComponent::new()));
    let _ = world.add_child(viz_root, overlay_root);
    world.init_component_tree(overlay_root, emit);
    overlay_root
}

fn immediate_owned_viz_root(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_BOX_MODEL_VIZ_ROOT)
            && world.get_component_by_id_as::<TransformComponent>(child).is_some()
    })
}

fn immediate_owned_overlay_root(world: &World, owner: ComponentId) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(OWNED_BOX_MODEL_VIZ_OVERLAY)
            && world.get_component_by_id_as::<OverlayComponent>(child).is_some()
    })
}

fn immediate_owned_viz_quad(world: &World, owner: ComponentId, label: &str) -> Option<ComponentId> {
    world.children_of(owner).iter().copied().find(|&child| {
        world.component_label(child) == Some(label)
            && world.get_component_by_id_as::<TransformComponent>(child).is_some()
    })
}

fn sync_optional_margin_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent: ComponentId,
    label: &str,
    left_gu: f32,
    top_gu: f32,
    width_gu: f32,
    height_gu: f32,
    marker_orientation: MarkerOrientation,
    unit_scale: f32,
) {
    if width_gu <= 0.0 || height_gu <= 0.0 {
        remove_owned_viz_quad(world, emit, parent, label);
        remove_owned_viz_quad(world, emit, parent, &marker_label(label));
        return;
    }

    sync_viz_quad(
        world,
        emit,
        parent,
        label,
        MARGIN_RGBA,
        left_gu,
        top_gu,
        width_gu,
        height_gu,
        Z_MARGIN,
        unit_scale,
    );

    sync_center_marker(
        world,
        emit,
        parent,
        &marker_label(label),
        left_gu,
        top_gu,
        width_gu,
        height_gu,
        marker_orientation,
        unit_scale,
    );
}

fn sync_optional_padding_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent: ComponentId,
    label: &str,
    left_gu: f32,
    top_gu: f32,
    width_gu: f32,
    height_gu: f32,
    marker_orientation: MarkerOrientation,
    unit_scale: f32,
) {
    if width_gu <= 0.0 || height_gu <= 0.0 {
        remove_owned_viz_quad(world, emit, parent, label);
        remove_owned_viz_quad(world, emit, parent, &marker_label(label));
        return;
    }

    sync_viz_quad(
        world,
        emit,
        parent,
        label,
        PADDING_RGBA,
        left_gu,
        top_gu,
        width_gu,
        height_gu,
        Z_PADDING,
        unit_scale,
    );

    sync_center_marker(
        world,
        emit,
        parent,
        &marker_label(label),
        left_gu,
        top_gu,
        width_gu,
        height_gu,
        marker_orientation,
        unit_scale,
    );
}

fn sync_center_marker(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent: ComponentId,
    label: &str,
    slab_left_gu: f32,
    slab_top_gu: f32,
    slab_width_gu: f32,
    slab_height_gu: f32,
    orientation: MarkerOrientation,
    unit_scale: f32,
) {
    let (marker_width_gu, marker_height_gu) = match orientation {
        MarkerOrientation::Vertical => (slab_width_gu.min(MARKER_NARROW_GU), slab_height_gu),
        MarkerOrientation::Horizontal => (slab_width_gu, slab_height_gu.min(MARKER_NARROW_GU)),
    };

    let marker_left_gu = slab_left_gu + (slab_width_gu - marker_width_gu) * 0.5;
    let marker_top_gu = slab_top_gu + (slab_height_gu - marker_height_gu) * 0.5;

    sync_viz_quad(
        world,
        emit,
        parent,
        label,
        MARKER_RGBA,
        marker_left_gu,
        marker_top_gu,
        marker_width_gu,
        marker_height_gu,
        Z_MARKER,
        unit_scale,
    );
}

fn marker_label(label: &str) -> String {
    format!("{label}__marker")
}

fn remove_owned_viz_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    owner: ComponentId,
    label: &str,
) {
    if let Some(existing) = immediate_owned_viz_quad(world, owner, label) {
        emit.push_intent_now(
            existing,
            IntentValue::RemoveSubtree { component_ids: vec![existing] },
        );
    }
}

fn sync_viz_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent: ComponentId,
    label: &str,
    rgba: [f32; 4],
    left_gu: f32,
    top_gu: f32,
    width_gu: f32,
    height_gu: f32,
    z: f32,
    unit_scale: f32,
) {
    if width_gu <= 0.0 || height_gu <= 0.0 {
        return;
    }

    let quad_id = match immediate_owned_viz_quad(world, parent, label) {
        Some(id) => id,
        None => spawn_viz_quad(world, emit, parent, label, rgba),
    };

    emit.push_intent_now(
        quad_id,
        IntentValue::UpdateTransform {
            component_ids: vec![quad_id],
            translation: [
                (left_gu + width_gu / 2.0 - 0.5) * unit_scale,
                -((top_gu + height_gu / 2.0 - 0.5) * unit_scale),
                z,
            ],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [width_gu * unit_scale, height_gu * unit_scale, 1.0],
        },
    );
}

fn spawn_viz_quad(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    parent: ComponentId,
    label: &str,
    rgba: [f32; 4],
) -> ComponentId {
    let quad_id = world.add_component_boxed_named(label, Box::new(TransformComponent::new()));
    let _ = world.add_child(parent, quad_id);

    let color_id = world.add_component(ColorComponent { rgba });
    let _ = world.add_child(quad_id, color_id);

    let renderable_id = world.add_component(RenderableComponent::square());
    let _ = world.add_child(color_id, renderable_id);

    if rgba[3] < 1.0 {
        let opacity_id = world.add_component(OpacityComponent::new().with_opacity(rgba[3]));
        let _ = world.add_child(renderable_id, opacity_id);
    }

    world.init_component_tree(quad_id, emit);
    quad_id
}