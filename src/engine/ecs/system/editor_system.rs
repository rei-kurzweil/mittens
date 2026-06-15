use crate::engine::ecs::component::{
    EditorComponent, EditorInteractionMode, RaycastableComponent, SelectableComponent,
    SerializeComponent, TransformComponent, TransformGizmoComponent,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor_scene_hit::resolve_editor_scene_hit;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, RxWorld, SignalKind, World};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const EDITOR_SELECT_HANDLER_NAME: &str = "editor_select";

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
        panel_query_root: ComponentId,
        editor_context_state: Arc<Mutex<EditorContextState>>,
    ) {
        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);

        rx.add_handler_closure_named(
            SignalKind::DragStart,
            editor_root,
            Some(EDITOR_SELECT_HANDLER_NAME.to_string()),
            move |world, emit, env| {
                let Some(EventSignal::DragStart { renderable, .. }) = env.event.as_ref() else {
                    return;
                };

                let editor_context = editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned")
                    .clone();
                if paint_panel_is_focused(world, panel_query_root, &editor_context) {
                    return;
                }

                let Some(scene_hit) = resolve_editor_scene_hit(world, *renderable) else {
                    return;
                };
                if scene_hit.editor_root != editor_root {
                    return;
                }

                let interaction_mode = editor_interaction_mode(world, editor_root);
                eprintln!(
                    "🖱️🧭🎛️ editor click resolved editor_root={editor_root:?} renderable={renderable:?} target_transform={:?} mode={interaction_mode:?}",
                    scene_hit.target_transform
                );

                match interaction_mode {
                    EditorInteractionMode::Select => {
                        select_editor_target(
                            world,
                            emit,
                            editor_root,
                            scene_hit.target_transform,
                            true,
                        );
                    }
                    EditorInteractionMode::Cursor3d => {}
                    EditorInteractionMode::SelectAndCursor => {
                        select_editor_target(
                            world,
                            emit,
                            editor_root,
                            scene_hit.target_transform,
                            true,
                        );
                    }
                }
            },
        );
    }

    /// Materialize editor-default pickability by wrapping each current immediate child of the
    /// editor root in a single `RaycastableComponent::enabled()` ancestor, unless that subtree
    /// root already explicitly opts in or out.
    pub fn materialize_editor_raycastables(
        &mut self,
        world: &mut World,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        editor_root: ComponentId,
    ) {
        let children: Vec<ComponentId> = world.children_of(editor_root).to_vec();

        for child in children {
            if subtree_root_has_explicit_raycastable(world, child)
                || subtree_root_has_selectable_off(world, child)
            {
                continue;
            }

            let wrapper = world.add_component_boxed_named(
                "editor_auto_raycastable",
                Box::new(RaycastableComponent::enabled()),
            );
            let wrapper_serialize = world.add_component_boxed_named(
                "editor_auto_raycastable_serialize",
                Box::new(SerializeComponent::off()),
            );

            if world.add_child(editor_root, wrapper).is_err() {
                continue;
            }
            let _ = world.add_child(wrapper, wrapper_serialize);
            if world.add_child(wrapper, child).is_err() {
                let _ = world.remove_component_subtree(wrapper);
                continue;
            }

            world.init_component_tree(wrapper, emit);
        }
    }
}

fn subtree_root_has_explicit_raycastable(world: &World, node: ComponentId) -> bool {
    world
        .get_component_by_id_as::<RaycastableComponent>(node)
        .is_some()
}

fn subtree_root_has_selectable_off(world: &World, node: ComponentId) -> bool {
    world
        .get_component_by_id_as::<SelectableComponent>(node)
        .map(|s| !s.enabled)
        .unwrap_or(false)
}

fn paint_panel_is_focused(
    world: &World,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
) -> bool {
    let Some(paint_panel_root) = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR)
    else {
        return false;
    };
    editor_context.focused_panel == Some(paint_panel_root)
}

fn editor_interaction_mode(world: &World, editor_root: ComponentId) -> EditorInteractionMode {
    world
        .get_component_by_id_as::<EditorComponent>(editor_root)
        .map(|editor| editor.interaction_mode)
        .unwrap_or(EditorInteractionMode::Select)
}

pub(crate) fn select_editor_target(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    editor_root: ComponentId,
    target_transform: ComponentId,
    update_repl_cwd: bool,
) {
    let interaction_mode = world
        .get_component_by_id_as::<EditorComponent>(editor_root)
        .map(|editor| editor.interaction_mode)
        .unwrap_or(EditorInteractionMode::Select);
    eprintln!(
        "🧲🛠️🐛 select_editor_target called editor_root={editor_root:?} target_transform={target_transform:?} mode={interaction_mode:?} update_repl_cwd={update_repl_cwd}"
    );

    let gizmo = resolve_editor_transform_gizmo(world, editor_root)
        .or_else(|| spawn_editor_transform_gizmo(world, emit, editor_root));

    if let Some(gizmo) = gizmo {
        emit.push_intent_now(
            editor_root,
            IntentValue::Attach {
                parents: vec![target_transform],
                child: gizmo,
            },
        );
    }

    if let Some(ed) = world.get_component_by_id_as_mut::<EditorComponent>(editor_root) {
        ed.selected = Some(target_transform);
    }

    emit.push_event(
        editor_root,
        EventSignal::SelectionChanged {
            selection_root: editor_root,
            mode: crate::engine::ecs::component::SelectionMode::Single,
            selected_entries: vec![crate::engine::ecs::component::SelectionEntry {
                index: None,
                component: target_transform,
            }],
            selected_component: Some(target_transform),
            selected_payload: Some(target_transform),
        },
    );

    if update_repl_cwd {
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
    }
}

fn spawn_editor_transform_gizmo(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    editor_root: ComponentId,
) -> Option<ComponentId> {
    // --- Transform gizmo ---
    // Create a tiny anchor transform under the editor root so the gizmo has a Transform
    // ancestor before it is first moved onto a clicked target.
    let anchor =
        world.add_component_boxed_named("editor_gizmo_anchor", Box::new(TransformComponent::new()));
    let _ = world.add_child(editor_root, anchor);
    let anchor_selectable = world.add_component_boxed_named(
        "editor_gizmo_anchor_selectable",
        Box::new(SelectableComponent::off()),
    );
    let _ = world.add_child(anchor, anchor_selectable);
    let anchor_serialize = world.add_component_boxed_named(
        "editor_gizmo_anchor_serialize",
        Box::new(SerializeComponent::off()),
    );
    let _ = world.add_child(anchor, anchor_serialize);

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

    Some(gizmo)
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

#[cfg(test)]
mod tests {
    use super::EditorSystem;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        EditorComponent, EditorInteractionMode, GLTFComponent, RenderableComponent,
        TransformComponent,
    };
    use crate::engine::ecs::system::BvhSystem;
    use crate::engine::ecs::system::editor::context::EditorContextState;
    use crate::engine::ecs::{EventSignal, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use std::sync::{Arc, Mutex};

    #[test]
    fn select_mode_selects_target_without_moving_cursor() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut editor_system = EditorSystem::new();

        let panel_root =
            world.add_component_boxed_named("panel_root", Box::new(TransformComponent::new()));
        let editor_root = world.add_component_boxed_named(
            "editor_root",
            Box::new(EditorComponent::new().with_interaction_mode(EditorInteractionMode::Select)),
        );
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let renderable =
            world.add_component_boxed_named("plane", Box::new(RenderableComponent::plane()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, renderable);

        let context = Arc::new(Mutex::new(EditorContextState::default()));
        editor_system.install_scoped_handlers_for_editor(
            &mut systems.rx,
            editor_root,
            panel_root,
            context.clone(),
        );

        systems.rx.push_event(
            renderable,
            EventSignal::DragStart {
                raycaster: renderable,
                renderable,
                hit_point: [0.0, 0.0, 0.0],
                ray_dir_world: [0.0, 0.0, -1.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 10_000);

        let state = context
            .lock()
            .expect("editor context mutex poisoned")
            .clone();
        assert_eq!(state.active_editor, None);
        assert_eq!(state.selected_component, None);
        assert_eq!(state.cursor_translation, None);
        assert_eq!(
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .expect("editor")
                .selected,
            Some(scene_root)
        );
    }

    #[test]
    fn materialize_editor_raycastables_covers_gltf_branches() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut editor_system = EditorSystem::new();

        let editor_root = world.add_component_boxed_named(
            "editor_root",
            Box::new(EditorComponent::new().with_interaction_mode(EditorInteractionMode::Select)),
        );
        let gltf_anchor =
            world.add_component_boxed_named("gltf_anchor", Box::new(TransformComponent::new()));
        let gltf = world.add_component_boxed_named(
            "gltf_component",
            Box::new(GLTFComponent::new("assets/models/test.glb")),
        );
        let spawned_node =
            world.add_component_boxed_named("spawned_node", Box::new(TransformComponent::new()));
        let renderable = world
            .add_component_boxed_named("mesh_renderable", Box::new(RenderableComponent::cube()));

        let _ = world.add_child(editor_root, gltf_anchor);
        let _ = world.add_child(gltf_anchor, gltf);
        let _ = world.add_child(gltf_anchor, spawned_node);
        let _ = world.add_child(spawned_node, renderable);

        editor_system.materialize_editor_raycastables(&mut world, &mut emit, editor_root);

        assert!(
            BvhSystem::renderable_is_raycastable(&world, renderable),
            "expected GLTF-backed editor renderables to inherit the editor auto-raycastable wrapper"
        );
    }
}
