use crate::engine::ecs::component::{
    OpacityComponent, SelectableComponent, SerializeComponent, TransformComponent,
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
    remove_preview_marker_children(world, preview_root);
}

pub fn cancel_preview(world: &mut World, preview_root: ComponentId) {
    let _ = world.remove_component_subtree(preview_root);
}

fn remove_preview_marker_children(world: &mut World, preview_root: ComponentId) {
    let preview_children: Vec<_> = world
        .children_of(preview_root)
        .iter()
        .copied()
        .filter(|&child| {
            matches!(
                world.component_label(child),
                Some("placement_preview_selectable")
                    | Some("placement_preview_serialize")
                    | Some("placement_preview_opacity")
            )
        })
        .collect();
    for child in preview_children {
        let _ = world.remove_component_leaf(child);
    }
}
