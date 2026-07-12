use crate::engine::ecs::component::{
    EditorComponent, SelectableComponent, TransformComponent, TransformGizmoComponent,
};
use crate::engine::ecs::{ComponentId, World};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldSceneHit {
    pub editor_root: Option<ComponentId>,
    pub target_renderable: ComponentId,
    pub target_transform: ComponentId,
}

fn debug_editor_scene_hit_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let v = std::env::var("CAT_DEBUG_EDITOR_SCENE_HIT").unwrap_or_default();
        let v = v.trim().to_ascii_lowercase();
        matches!(v.as_str(), "1" | "true" | "yes" | "on")
    })
}

fn debug_component_label(world: &World, component: ComponentId) -> String {
    world
        .get_component_record(component)
        .map(|n| {
            if n.name.is_empty() {
                n.component_type.clone()
            } else {
                format!("{}: {}", n.component_type, n.name)
            }
        })
        .unwrap_or_else(|| "<missing>".to_string())
}

pub fn resolve_world_scene_hit(world: &World, renderable: ComponentId) -> Option<WorldSceneHit> {
    let editor_root = nearest_editor_ancestor(world, renderable);
    let blocked_by_selectable = has_selectable_off_ancestor(world, renderable);
    let blocked_by_gizmo = has_transform_gizmo_ancestor(world, renderable);
    if blocked_by_selectable || blocked_by_gizmo {
        if debug_editor_scene_hit_enabled() {
            println!(
                "[WorldSceneHit] reject renderable={renderable:?} '{}' editor_root={editor_root:?} selectable_off={} gizmo_ancestor={}",
                debug_component_label(world, renderable),
                blocked_by_selectable,
                blocked_by_gizmo,
            );
        }
        return None;
    }
    let target_transform = preferred_scene_selection_transform(world, renderable)
        .or_else(|| nearest_transform_ancestor(world, renderable))?;
    if debug_editor_scene_hit_enabled() {
        println!(
            "[WorldSceneHit] accept renderable={renderable:?} '{}' -> target_transform={target_transform:?} '{}' editor_root={editor_root:?}",
            debug_component_label(world, renderable),
            debug_component_label(world, target_transform),
        );
    }
    Some(WorldSceneHit {
        editor_root,
        target_renderable: renderable,
        target_transform,
    })
}

fn preferred_scene_selection_transform(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world.component_label(node) == Some("painted_asset_root") {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

pub fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<EditorComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

pub fn nearest_transform_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<TransformComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

pub fn has_transform_gizmo_ancestor(world: &World, start: ComponentId) -> bool {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<TransformGizmoComponent>(node)
            .is_some()
        {
            return true;
        }
        cur = world.parent_of(node);
    }
    false
}

pub fn has_selectable_off_ancestor(world: &World, start: ComponentId) -> bool {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if has_selectable_off_marker(world, node) {
            return true;
        }
        cur = world.parent_of(node);
    }
    false
}

fn has_selectable_off_marker(world: &World, component_id: ComponentId) -> bool {
    world
        .get_component_by_id_as::<SelectableComponent>(component_id)
        .map(|s| !s.enabled)
        .unwrap_or(false)
        || world.children_of(component_id).iter().copied().any(|child| {
            world
                .get_component_by_id_as::<SelectableComponent>(child)
                .map(|s| !s.enabled)
                .unwrap_or(false)
        })
}

#[cfg(test)]
mod tests {
    use super::resolve_world_scene_hit;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        EditorComponent, RenderableComponent, SelectableComponent, TransformComponent,
    };

    #[test]
    fn painted_asset_hits_resolve_to_wrapper_root() {
        let mut world = World::default();
        let editor = world.add_component(EditorComponent::new());
        let raycastable_root = world.add_component_boxed_named(
            "painted_asset_raycastable",
            Box::new(TransformComponent::new()),
        );
        let painted_root = world
            .add_component_boxed_named("painted_asset_root", Box::new(TransformComponent::new()));
        let internal = world
            .add_component_boxed_named("internal_mesh_root", Box::new(TransformComponent::new()));
        let renderable = world.add_component(RenderableComponent::cube());

        world
            .add_child(editor, raycastable_root)
            .expect("attach raycastable");
        world
            .add_child(raycastable_root, painted_root)
            .expect("attach painted root");
        world
            .add_child(painted_root, internal)
            .expect("attach internal");
        world
            .add_child(internal, renderable)
            .expect("attach renderable");

        let hit = resolve_world_scene_hit(&world, renderable).expect("scene hit");
        assert_eq!(hit.target_transform, painted_root);
        assert_eq!(hit.editor_root, Some(editor));
    }

    #[test]
    fn selectable_off_child_on_ancestor_blocks_scene_hit() {
        let mut world = World::default();
        let editor = world.add_component(EditorComponent::new());
        let scene_root = world.add_component(TransformComponent::new());
        let selectable = world.add_component(SelectableComponent::off());
        let child = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::cube());

        world.add_child(editor, scene_root).expect("attach scene root");
        world
            .add_child(scene_root, selectable)
            .expect("attach selectable marker");
        world.add_child(scene_root, child).expect("attach child");
        world
            .add_child(child, renderable)
            .expect("attach renderable");

        assert!(resolve_world_scene_hit(&world, renderable).is_none());
    }
}
