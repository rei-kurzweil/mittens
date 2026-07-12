use crate::engine::ecs::component::{
    OpacityComponent, SelectableComponent,
    SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::paint_placement::{PlacementPose, SurfacePlacementFrame};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

pub const PLACEMENT_PREVIEW_OPACITY: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementKind {
    PaintAsset,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementPreviewStyle {
    pub opacity: f32,
}

impl Default for PlacementPreviewStyle {
    fn default() -> Self {
        Self {
            opacity: PLACEMENT_PREVIEW_OPACITY,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementPreviewSession {
    pub active_editor: ComponentId,
    pub placement_kind: PlacementKind,
    pub preview_root_component_id: ComponentId,
    pub target_renderable: Option<ComponentId>,
    pub last_valid_placement_frame: Option<SurfacePlacementFrame>,
    pub local_min_z: f32,
}

pub fn create_preview_shell(
    world: &mut World,
    preview_root: ComponentId,
    emit: &mut dyn SignalEmitter,
    style: PlacementPreviewStyle,
) {
    let selectable = world.add_component_boxed_named(
        "placement_preview_selectable",
        Box::new(SelectableComponent::off()),
    );
    let serialize = world.add_component_boxed_named(
        "placement_preview_serialize",
        Box::new(SerializeComponent::off()),
    );
    let opacity = world.add_component_boxed_named(
        "placement_preview_opacity",
        Box::new(OpacityComponent::new().with_opacity(style.opacity)),
    );
    let _ = world.add_child(preview_root, selectable);
    let _ = world.add_child(preview_root, serialize);
    let _ = world.add_child(preview_root, opacity);
    world.init_component_tree(preview_root, emit);
}

pub fn update_preview_pose(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    preview_root: ComponentId,
    pose: PlacementPose,
) {
    let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(preview_root)
    else {
        return;
    };
    transform.transform.translation = pose.translation;
    transform.transform.rotation = pose.rotation;
    transform.transform.recompute_model();
    emit.push_intent_now(
        preview_root,
        IntentValue::UpdateTransform {
            component_ids: vec![preview_root],
            translation: pose.translation,
            rotation_quat_xyzw: pose.rotation,
            scale: transform.transform.scale,
        },
    );
}

pub fn commit_preview(world: &mut World, preview_root: ComponentId) {
    remove_preview_markers(world, preview_root);
}

pub fn cancel_preview(world: &mut World, preview_root: ComponentId) {
    let _ = world.remove_component_subtree(preview_root);
}

fn remove_preview_markers(world: &mut World, preview_root: ComponentId) {
    let mut stack = vec![preview_root];
    let mut preview_nodes = Vec::new();
    while let Some(node) = stack.pop() {
        if matches!(
            world.component_label(node),
            Some("placement_preview_selectable")
                | Some("placement_preview_serialize")
                | Some("placement_preview_world_panel")
                | Some("placement_preview_opacity")
        ) {
            preview_nodes.push(node);
            continue;
        }
        for &child in world.children_of(node) {
            stack.push(child);
        }
    }
    for child in preview_nodes {
        let _ = world.remove_component_leaf(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_preview_removes_preview_markers_recursively() {
        let mut world = World::default();
        let preview_root = world.add_component(TransformComponent::new());
        let child = world.add_component(TransformComponent::new());
        let grandchild = world.add_component(TransformComponent::new());
        let opacity = world.add_component_boxed_named(
            "placement_preview_opacity",
            Box::new(OpacityComponent::new().with_opacity(PLACEMENT_PREVIEW_OPACITY)),
        );
        let selectable = world.add_component_boxed_named(
            "placement_preview_selectable",
            Box::new(SelectableComponent::off()),
        );

        let _ = world.add_child(preview_root, child);
        let _ = world.add_child(child, grandchild);
        let _ = world.add_child(grandchild, opacity);
        let _ = world.add_child(preview_root, selectable);

        commit_preview(&mut world, preview_root);

        assert!(world.get_component_record(opacity).is_none());
        assert!(world.get_component_record(selectable).is_none());
        assert!(world.get_component_record(child).is_some());
        assert!(world.get_component_record(grandchild).is_some());
    }
}
