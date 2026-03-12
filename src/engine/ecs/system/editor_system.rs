use crate::engine::ecs::component::{EditorComponent, TransformComponent, TransformGizmoComponent};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, RxWorld, SignalKind, World};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct EditorSystem {
    installed_editor_roots: HashSet<ComponentId>,
}

impl EditorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install gesture/picking handlers scoped to a specific editor root subtree.
    ///
    /// This allows multiple EditorComponent subtrees, each with independent selection/gizmo state.
    pub fn install_scoped_handlers_for_editor(
        &mut self,
        rx: &mut RxWorld,
        editor_root: ComponentId,
    ) {
        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);

        rx.add_handler_closure(SignalKind::DragStart, editor_root, move |world, emit, env| {
            let Some(EventSignal::DragStart { renderable, .. }) = env.event.as_ref() else {
                return;
            };

            let renderable = renderable.clone();

            // If editors are nested, only the *nearest* editor root should handle the event.
            if nearest_editor_ancestor(world, renderable) != Some(editor_root) {
                return;
            }

            // Ignore clicks on transform gizmo handles themselves.
            if has_transform_gizmo_ancestor(world, renderable) {
                return;
            }

            // Resolve the clicked target's nearest TransformComponent.
            let Some(target_transform) = nearest_transform_ancestor(world, renderable) else {
                return;
            };

            // Resolve (or discover) the editor's TransformGizmo.
            let gizmo = resolve_editor_transform_gizmo(world, editor_root)
                .or_else(|| spawn_editor_transform_gizmo(world, emit, editor_root));
            let Some(gizmo) = gizmo else { return; };

            let renderable_name = world
                .get_component_record(renderable)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| "<unknown>".to_string());
            let target_name = world
                .get_component_record(target_transform)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| "<unknown>".to_string());
            let old_parent = world.parent_of(gizmo);
            let old_parent_name = old_parent
                .and_then(|p| world.get_component_record(p).map(|r| r.name.clone()))
                .unwrap_or_else(|| "<none>".to_string());
            println!(
                "[EditorSystem] DragStart hit renderable={:?} name='{}' -> moving gizmo={:?} (old_parent={:?} '{}') under target_transform={:?} '{}'",
                renderable,
                renderable_name,
                gizmo,
                old_parent,
                old_parent_name,
                target_transform,
                target_name
            );

            // Reparent the gizmo under the clicked transform.
            emit.push_intent_now(
                editor_root,
                IntentValue::Attach {
                    parents: vec![target_transform],
                    child: gizmo,
                },
            );

            // Jump the REPL cwd to the clicked target so we can inspect topology quickly.
            if let Some(node) = world.get_component_node(target_transform) {
                emit.push_intent_now(
                    editor_root,
                    IntentValue::ReplExec {
                        command: format!("cd {}", node.guid),
                    },
                );
                emit.push_intent_now(
                    editor_root,
                    IntentValue::ReplExec {
                        command: "pwd".to_string(),
                    },
                );
            }
        });
    }
}

fn spawn_editor_transform_gizmo(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    editor_root: ComponentId,
) -> Option<ComponentId> {
    // Create a tiny anchor transform under the eot so the gizmo has a Transform ancestor
    // before it is first moved onto a clicked target.
    let anchor =
        world.add_component_boxed_named("editor_gizmo_anchor", Box::new(TransformComponent::new()));
    let _ = world.add_child(editor_root, anchor);

    // Interpret `scale` as world-space size (GizmoSystem compensates for inherited scales).
    let gizmo = world.add_component_boxed_named(
        "editor_transform_gizmo",
        Box::new(TransformGizmoComponent::new().with_scale(0.5)),
    );
    let _ = world.add_child(anchor, gizmo);

    // Initialize to trigger RegisterTransformGizmo and spawn visuals.
    world.init_component_tree(anchor, emit);

    if let Some(ed) = world.get_component_by_id_as_mut::<EditorComponent>(editor_root) {
        ed.transform_gizmo = Some(gizmo);
    }

    println!(
        "[EditorSystem] spawned editor transform gizmo={:?} under anchor={:?} (editor_root={:?})",
        gizmo, anchor, editor_root
    );

    Some(gizmo)
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
