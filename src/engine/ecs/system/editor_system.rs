use crate::engine::ecs::component::{EditorComponent, TransformComponent, TransformGizmoComponent};
use crate::engine::ecs::{ComponentId, RxWorld, SignalKind, SignalValue, World};

#[derive(Debug, Default)]
pub struct EditorSystem {
    immediate_handlers_installed: bool,
}

impl EditorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.immediate_handlers_installed {
            return;
        }

        rx.add_global_handler_closure(SignalKind::DragStart, move |world, emit, env| {
            let SignalValue::DragStart { renderable, .. } = &env.value else {
                return;
            };

            let renderable = renderable.clone();

            // Ignore clicks on transform gizmo handles themselves.
            if has_transform_gizmo_ancestor(world, renderable) {
                return;
            }

            // Only act when the clicked renderable lives under an editor root.
            let Some(editor_root) = nearest_editor_ancestor(world, renderable) else {
                return;
            };

            // Resolve the clicked target's nearest TransformComponent.
            let Some(target_transform) = nearest_transform_ancestor(world, renderable) else {
                return;
            };

            // Resolve (or discover) the editor's TransformGizmo.
            let gizmo = resolve_editor_transform_gizmo(world, editor_root);
            let Some(gizmo) = gizmo else {
                return;
            };

            // Reparent the gizmo under the clicked transform via ActionSystem.
            emit.push(
                editor_root,
                SignalValue::Attach {
                    parents: vec![target_transform],
                    child: gizmo,
                },
            );
        });

        self.immediate_handlers_installed = true;
    }
}

fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
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

fn nearest_transform_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
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

fn has_transform_gizmo_ancestor(world: &World, start: ComponentId) -> bool {
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

fn resolve_editor_transform_gizmo(
    world: &mut World,
    editor_root: ComponentId,
) -> Option<ComponentId> {
    // Fast path: cached id.
    if let Some(ed) = world.get_component_by_id_as::<EditorComponent>(editor_root) {
        if let Some(g) = ed.transform_gizmo {
            if world
                .get_component_by_id_as::<TransformGizmoComponent>(g)
                .is_some()
            {
                return Some(g);
            }
        }
    }

    let found = find_transform_gizmo_in_subtree(world, editor_root);

    if let Some(ed) = world.get_component_by_id_as_mut::<EditorComponent>(editor_root) {
        ed.transform_gizmo = found;
    }

    found
}

fn find_transform_gizmo_in_subtree(world: &World, root: ComponentId) -> Option<ComponentId> {
    let mut stack: Vec<ComponentId> = vec![root];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<TransformGizmoComponent>(node)
            .is_some()
        {
            return Some(node);
        }

        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
    None
}
