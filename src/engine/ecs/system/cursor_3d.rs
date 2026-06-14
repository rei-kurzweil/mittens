use crate::engine::ecs::component::EditorInteractionMode;
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor_scene_hit::resolve_editor_scene_hit;
use crate::engine::ecs::system::paint_placement::resolve_surface_aligned_pose_for_subtree;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, RxWorld, SignalKind, World};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

const EDITOR_CURSOR_HANDLER_NAME: &str = "editor_cursor_3d";
const CURSOR_MARKER_ROOT_NAME: &str = "editor_cursor_marker";
const CURSOR_MARKER_SIZE: f32 = 0.5;
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";

#[derive(Debug, Default)]
pub struct Cursor3dSystem {
    installed_editor_roots: HashSet<ComponentId>,
}

impl Cursor3dSystem {
    pub fn new() -> Self {
        Self::default()
    }

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
            Some(EDITOR_CURSOR_HANDLER_NAME.to_string()),
            move |world, emit, env| {
                eprintln!("✨✨✨🐈🐈🐈 cursor_3d handler invoked editor_root={editor_root:?}");
                let Some(EventSignal::DragStart {
                    renderable,
                    hit_point,
                    ..
                }) = env.event.as_ref()
                else {
                    eprintln!("✨✨✨🐈🐈🐈 cursor_3d ignoring non-DragStart signal editor_root={editor_root:?}");
                    return;
                };

                eprintln!(
                    "✨✨✨🐈🐈🐈 cursor_3d drag_start editor_root={editor_root:?} renderable={renderable:?} hit_point={hit_point:?}"
                );

                let editor_context = editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned")
                    .clone();
                if paint_panel_is_focused(world, panel_query_root, &editor_context) {
                    eprintln!(
                        "✨✨✨🐈🐈🐈 cursor_3d suppressed because paint panel is focused editor_root={editor_root:?} focused_panel={:?}",
                        editor_context.focused_panel
                    );
                    return;
                }

                let Some(scene_hit) = resolve_editor_scene_hit(world, *renderable) else {
                    eprintln!(
                        "✨✨✨🐈🐈🐈 cursor_3d failed resolve_editor_scene_hit editor_root={editor_root:?} renderable={renderable:?}"
                    );
                    return;
                };
                if scene_hit.editor_root != editor_root {
                    eprintln!(
                        "✨✨✨🐈🐈🐈 cursor_3d scene_hit belongs to different editor requested_editor={editor_root:?} hit_editor={:?} renderable={renderable:?}",
                        scene_hit.editor_root
                    );
                    return;
                }

                eprintln!(
                    "✨✨✨🐈🐈🐈 cursor_3d resolved scene_hit editor_root={editor_root:?} target_renderable={:?} target_transform={:?}",
                    scene_hit.target_renderable,
                    scene_hit.target_transform
                );

                update_editor_cursor_from_surface(
                    world,
                    emit,
                    editor_context_state.clone(),
                    editor_root,
                    scene_hit.target_renderable,
                    *hit_point,
                );
            },
        );
    }
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

fn update_editor_cursor_from_surface(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    editor_context_state: Arc<Mutex<EditorContextState>>,
    editor_root: ComponentId,
    target_renderable: ComponentId,
    hit_point: [f32; 3],
) {
    eprintln!(
        "✨✨✨🐈🐈🐈 cursor_3d update_from_surface begin editor_root={editor_root:?} target_renderable={target_renderable:?} hit_point={hit_point:?}"
    );
    let marker_root = world
        .children_of(editor_root)
        .iter()
        .copied()
        .find(|&child| world.component_label(child) == Some(CURSOR_MARKER_ROOT_NAME));
    let Some(marker_root) = marker_root else {
        eprintln!(
            "✨✨✨🐈🐈🐈 cursor_3d missing marker_root editor_root={editor_root:?} expected_label={CURSOR_MARKER_ROOT_NAME}"
        );
        return;
    };

    eprintln!(
        "✨✨✨🐈🐈🐈 cursor_3d found marker_root editor_root={editor_root:?} marker_root={marker_root:?}"
    );

    let Ok(pose) = resolve_surface_aligned_pose_for_subtree(
        world,
        target_renderable,
        hit_point,
        marker_root,
        None,
    ) else {
        eprintln!(
            "✨✨✨🐈🐈🐈 cursor_3d resolve_surface_aligned_pose_for_subtree failed editor_root={editor_root:?} marker_root={marker_root:?} target_renderable={target_renderable:?} hit_point={hit_point:?}"
        );
        return;
    };

    eprintln!(
        "✨✨✨🐈🐈🐈 cursor_3d pose resolved editor_root={editor_root:?} marker_root={marker_root:?} translation={:?} rotation={:?}",
        pose.translation,
        pose.rotation
    );

    {
        let mut editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned");
        editor_context.active_editor = Some(editor_root);
        editor_context.interaction_mode = match editor_context.interaction_mode {
            EditorInteractionMode::Select => EditorInteractionMode::Select,
            EditorInteractionMode::Cursor3d => EditorInteractionMode::Cursor3d,
            EditorInteractionMode::SelectAndCursor => EditorInteractionMode::SelectAndCursor,
        };
        editor_context.cursor_translation = Some(pose.translation);
        editor_context.cursor_rotation = Some(pose.rotation);
    }

    emit.push_intent_now(
        marker_root,
        IntentValue::UpdateTransform {
            component_ids: vec![marker_root],
            translation: pose.translation,
            rotation_quat_xyzw: pose.rotation,
            scale: [CURSOR_MARKER_SIZE, CURSOR_MARKER_SIZE, CURSOR_MARKER_SIZE],
        },
    );
    eprintln!(
        "✨✨✨🐈🐈🐈 cursor_3d emitted UpdateTransform marker_root={marker_root:?} translation={:?} rotation={:?}",
        pose.translation,
        pose.rotation
    );
}

#[cfg(test)]
mod tests {
    use super::Cursor3dSystem;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        EditorComponent, EditorInteractionMode, RenderableComponent, TransformComponent,
    };
    use crate::engine::ecs::system::editor::context::EditorContextSystem;
    use crate::engine::ecs::system::editor_system::EditorSystem;
    use crate::engine::ecs::{EventSignal, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn cursor_mode_places_cursor_without_selecting() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut context_system = EditorContextSystem::new();
        let mut cursor_system = Cursor3dSystem::new();

        let panel_root =
            world.add_component_boxed_named("panel_root", Box::new(TransformComponent::new()));
        let editor_root = world.add_component_boxed_named(
            "editor_root",
            Box::new(EditorComponent::new().with_interaction_mode(EditorInteractionMode::Cursor3d)),
        );
        context_system.install_scoped_handlers_for_editor(
            &mut systems.rx,
            &mut world,
            editor_root,
            panel_root,
        );
        let context = context_system.shared_state();
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let renderable =
            world.add_component_boxed_named("cube", Box::new(RenderableComponent::cube()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, renderable);
        cursor_system.install_scoped_handlers_for_editor(
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
                hit_point: [0.5, 0.0, 0.0],
                ray_dir_world: [-1.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 10_000);

        let state = context
            .lock()
            .expect("editor context mutex poisoned")
            .clone();
        assert_eq!(state.active_editor, Some(editor_root));
        assert_eq!(state.selected_component, Some(editor_root));
        assert_eq!(state.interaction_mode, EditorInteractionMode::Cursor3d);
        assert!(state.cursor_translation.is_some());
        assert!(state.cursor_rotation.is_some());
        assert_eq!(
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .expect("editor")
                .selected,
            None
        );
    }

    #[test]
    fn select_and_cursor_mode_performs_both_actions() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut context_system = EditorContextSystem::new();
        let mut cursor_system = Cursor3dSystem::new();
        let mut editor_system = EditorSystem::new();

        let panel_root =
            world.add_component_boxed_named("panel_root", Box::new(TransformComponent::new()));
        let editor_root = world.add_component_boxed_named(
            "editor_root",
            Box::new(
                EditorComponent::new()
                    .with_interaction_mode(EditorInteractionMode::SelectAndCursor),
            ),
        );
        context_system.install_scoped_handlers_for_editor(
            &mut systems.rx,
            &mut world,
            editor_root,
            panel_root,
        );
        let context = context_system.shared_state();
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let renderable =
            world.add_component_boxed_named("plane", Box::new(RenderableComponent::plane()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, renderable);
        editor_system.install_scoped_handlers_for_editor(
            &mut systems.rx,
            editor_root,
            panel_root,
            context.clone(),
        );
        cursor_system.install_scoped_handlers_for_editor(
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
        assert_eq!(state.active_editor, Some(editor_root));
        assert_eq!(
            state.interaction_mode,
            EditorInteractionMode::SelectAndCursor
        );
        assert_eq!(
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .expect("editor")
                .selected,
            Some(scene_root)
        );
        assert!(state.cursor_translation.is_some());
        assert!(state.cursor_rotation.is_some());
    }
}
