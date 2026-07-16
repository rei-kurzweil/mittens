use crate::engine::ecs::component::{
    EditorComponent, EditorInteractionMode, RaycastableComponent, SelectableComponent,
    SerializeComponent,
};
use crate::engine::ecs::system::editor::context::{
    EditorContextState, ensure_shared_workspace_transform_gizmo_global,
};
use crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, RxWorld, SignalKind, World};
use std::collections::HashSet;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

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
        _panel_query_root: ComponentId,
        editor_context_state: Arc<Mutex<EditorContextState>>,
    ) {
        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);

        let scoped_editor_context_state = editor_context_state.clone();
        rx.add_handler_closure_named(
            SignalKind::Click,
            editor_root,
            Some(EDITOR_SELECT_HANDLER_NAME.to_string()),
            move |world, emit, env| {
                let Some(EventSignal::Click { renderable, .. }) = env.event.as_ref() else {
                    return;
                };

                let Some(scene_hit) = resolve_world_scene_hit(world, *renderable) else {
                    if debug_editor_select_enabled() {
                        eprintln!(
                            "[editor_select] no scene hit renderable={:?} '{}'",
                            renderable,
                            debug_component_label(world, *renderable)
                        );
                    }
                    return;
                };
                let editor_context = scoped_editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned")
                    .clone();
                let handles_non_editor_hit = scene_hit.editor_root.is_none()
                    && editor_context.active_editor == Some(editor_root);
                if scene_hit.editor_root != Some(editor_root) && !handles_non_editor_hit {
                    if debug_editor_select_enabled() {
                        eprintln!(
                            "[editor_select] reject renderable={:?} '{}' scene_editor={:?} active_editor={:?} editor_root={:?}",
                            renderable,
                            debug_component_label(world, *renderable),
                            scene_hit.editor_root,
                            editor_context.active_editor,
                            editor_root
                        );
                    }
                    return;
                }
                if debug_editor_select_enabled() {
                    eprintln!(
                        "[editor_select] select renderable={:?} '{}' target_transform={:?} '{}' mode={:?}",
                        renderable,
                        debug_component_label(world, *renderable),
                        scene_hit.target_transform,
                        debug_component_label(world, scene_hit.target_transform),
                        editor_interaction_mode(world, editor_root)
                    );
                }

                let interaction_mode = editor_interaction_mode(world, editor_root);
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

        let global_editor_context_state = editor_context_state.clone();
        rx.add_global_handler_closure_named(
            SignalKind::Click,
            Some(format!("{EDITOR_SELECT_HANDLER_NAME}_global_{editor_root:?}")),
            move |world, emit, env| {
                let Some(EventSignal::Click { renderable, .. }) = env.event.as_ref() else {
                    return;
                };

                let Some(scene_hit) = resolve_world_scene_hit(world, *renderable) else {
                    return;
                };
                if scene_hit.editor_root.is_some() {
                    return;
                }

                let editor_context = global_editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned")
                    .clone();
                if editor_context
                    .active_editor
                    .is_some_and(|active_editor| active_editor != editor_root)
                {
                    return;
                }

                if debug_editor_select_enabled() {
                    eprintln!(
                        "[editor_select] global select renderable={:?} '{}' target_transform={:?} '{}'",
                        renderable,
                        debug_component_label(world, *renderable),
                        scene_hit.target_transform,
                        debug_component_label(world, scene_hit.target_transform),
                    );
                }

                match editor_interaction_mode(world, editor_root) {
                    EditorInteractionMode::Select | EditorInteractionMode::SelectAndCursor => {
                        select_editor_target(
                            world,
                            emit,
                            editor_root,
                            scene_hit.target_transform,
                            true,
                        );
                    }
                    EditorInteractionMode::Cursor3d => {}
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

fn debug_editor_select_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let v = std::env::var("CAT_DEBUG_EDITOR_SELECT").unwrap_or_default();
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
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

    let gizmo = ensure_shared_workspace_transform_gizmo_global(world, emit);

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
        let mut render_assets = RenderAssets::new();
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
            EventSignal::Click {
                raycaster: renderable,
                renderable,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            10_000,
        );

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
