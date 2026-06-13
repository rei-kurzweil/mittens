use crate::engine::ecs::component::{
    EditorComponent, SelectableComponent, TransformComponent, TransformGizmoComponent,
};
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSceneHit {
    pub editor_root: ComponentId,
    pub target_renderable: ComponentId,
    pub target_transform: ComponentId,
}

pub fn resolve_editor_scene_hit(world: &World, renderable: ComponentId) -> Option<EditorSceneHit> {
    let editor_root = nearest_editor_ancestor(world, renderable)?;
    if has_selectable_off_ancestor(world, renderable)
        || has_transform_gizmo_ancestor(world, renderable)
    {
        return None;
    }
    let target_transform = nearest_transform_ancestor(world, renderable)?;
    Some(EditorSceneHit {
        editor_root,
        target_renderable: renderable,
        target_transform,
    })
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
        if world
            .get_component_by_id_as::<SelectableComponent>(node)
            .map(|s| !s.enabled)
            .unwrap_or(false)
        {
            return true;
        }
        cur = world.parent_of(node);
    }
    false
}
