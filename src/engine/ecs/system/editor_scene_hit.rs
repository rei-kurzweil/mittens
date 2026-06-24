use crate::engine::ecs::component::{
    EditorComponent, SelectableComponent, TransformComponent, TransformGizmoComponent,
};
use crate::engine::ecs::{ComponentId, World};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSceneHit {
    pub editor_root: ComponentId,
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
    world.get_component_record(component)
        .map(|n| {
            if n.name.is_empty() {
                n.component_type.clone()
            } else {
                format!("{}: {}", n.component_type, n.name)
            }
        })
        .unwrap_or_else(|| "<missing>".to_string())
}

pub fn resolve_editor_scene_hit(world: &World, renderable: ComponentId) -> Option<EditorSceneHit> {
    let editor_root = nearest_editor_ancestor(world, renderable)?;
    let blocked_by_selectable = has_selectable_off_ancestor(world, renderable);
    let blocked_by_gizmo = has_transform_gizmo_ancestor(world, renderable);
    if blocked_by_selectable || blocked_by_gizmo {
        if debug_editor_scene_hit_enabled() {
            println!(
                "[EditorSceneHit] reject renderable={renderable:?} '{}' editor_root={editor_root:?} '{}' selectable_off={} gizmo_ancestor={}",
                debug_component_label(world, renderable),
                debug_component_label(world, editor_root),
                blocked_by_selectable,
                blocked_by_gizmo,
            );
        }
        return None;
    }
    let target_transform = nearest_transform_ancestor(world, renderable)?;
    if debug_editor_scene_hit_enabled() {
        println!(
            "[EditorSceneHit] accept renderable={renderable:?} '{}' -> target_transform={target_transform:?} '{}' editor_root={editor_root:?} '{}'",
            debug_component_label(world, renderable),
            debug_component_label(world, target_transform),
            debug_component_label(world, editor_root),
        );
    }
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
