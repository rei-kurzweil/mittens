use crate::engine::ecs::component::EditorInteractionMode;
use crate::engine::ecs::system::editor::context::{EditorContextState, sync_editor_cursor_visual};
use crate::engine::ecs::system::editor_scene_hit::resolve_editor_scene_hit;
use crate::engine::ecs::system::paint_placement::{
    resolve_surface_aligned_pose_from_frame, resolve_surface_placement_frame,
};
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalKind, World};
use crate::utils::math;
use std::collections::HashSet;
use std::f32::consts::FRAC_PI_2;
use std::sync::{Arc, Mutex};

const EDITOR_CURSOR_HANDLER_NAME: &str = "editor_cursor_3d";
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

        for signal_kind in [SignalKind::DragStart] {
            let editor_context_state = editor_context_state.clone();
            rx.add_handler_closure_named(
                signal_kind,
                editor_root,
                Some(EDITOR_CURSOR_HANDLER_NAME.to_string()),
                move |world, emit, env| {
                    handle_cursor_signal(
                        world,
                        emit,
                        env.event.as_ref(),
                        editor_root,
                        panel_query_root,
                        editor_context_state.clone(),
                    );
                },
            );
        }
    }
}

fn handle_cursor_signal(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    event: Option<&EventSignal>,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    editor_context_state: Arc<Mutex<EditorContextState>>,
) {
    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();
    if paint_panel_is_focused(world, panel_query_root, &editor_context) {
        return;
    }

    match event {
        Some(EventSignal::DragStart {
            renderable,
            hit_point,
            ..
        }) => {
            let Some(scene_hit) = resolve_editor_scene_hit(world, *renderable) else {
                return;
            };
            if scene_hit.editor_root != editor_root {
                return;
            }
            update_editor_cursor(
                world,
                emit,
                editor_context_state,
                editor_root,
                panel_query_root,
                scene_hit.target_transform,
                scene_hit.target_renderable,
                *hit_point,
            );
        }
        _ => {}
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

fn update_editor_cursor(
    world: &mut World,
    emit: &mut dyn crate::engine::ecs::SignalEmitter,
    editor_context_state: Arc<Mutex<EditorContextState>>,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    target_transform: ComponentId,
    target_renderable: ComponentId,
    hit_point: [f32; 3],
) {
    let interaction_mode = world
        .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(editor_root)
        .map(|editor| editor.interaction_mode)
        .unwrap_or(EditorInteractionMode::Select);

    let (translation, rotation, frame) = match interaction_mode {
        EditorInteractionMode::Select => return,
        EditorInteractionMode::Cursor3d => {
            let Ok(frame) =
                resolve_surface_placement_frame(world, target_renderable, hit_point, None)
            else {
                return;
            };
            let Ok(pose) = resolve_surface_aligned_pose_from_frame(&frame, -0.25) else {
                return;
            };
            (
                pose.translation,
                remap_cursor_rotation_to_surface_up(pose.rotation),
                Some(frame),
            )
        }
        EditorInteractionMode::SelectAndCursor => {
            let Some(model) = authored_world_model(world, target_transform) else {
                return;
            };
            (
                [model[3][0], model[3][1], model[3][2]],
                math::mat_to_quat(model),
                None,
            )
        }
    };

    {
        let mut editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned");
        editor_context.active_editor = Some(editor_root);
        editor_context.interaction_mode = interaction_mode;
        editor_context.cursor_translation = Some(translation);
        editor_context.cursor_rotation = Some(rotation);
        editor_context.cursor_frame = frame;
    }

    sync_editor_cursor_visual(world, emit, &editor_context_state, Some(panel_query_root));
}

fn remap_cursor_rotation_to_surface_up(surface_aligned_rotation: [f32; 4]) -> [f32; 4] {
    let z_to_y = math::quat_from_axis_angle([1.0, 0.0, 0.0], FRAC_PI_2);
    math::quat_mul(surface_aligned_rotation, z_to_y)
}

fn authored_world_model(
    world: &World,
    component: ComponentId,
) -> Option<crate::engine::graphics::primitives::TransformMatrix> {
    let mut chain = Vec::new();

    if world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(component)
        .is_some()
    {
        chain.push(component);
    }

    let mut current = component;
    while let Some(parent) = world.parent_of(current) {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            .is_some()
        {
            chain.push(parent);
        }
        current = parent;
    }

    let mut iter = chain.into_iter().rev();
    let first = iter.next()?;
    let mut world_model = world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(first)?
        .transform
        .model;

    for transform_id in iter {
        let local = world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                transform_id,
            )?
            .transform
            .model;
        world_model = math::mat4_mul(world_model, local);
    }

    Some(world_model)
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
    use crate::utils::math::quat_rotate_vec3;

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

        systems.rx.push_event(
            renderable,
            EventSignal::Click {
                raycaster: renderable,
                renderable,
                hit_point: [0.0, 0.0, 0.0],
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
        let cursor_up = quat_rotate_vec3(
            state.cursor_rotation.expect("cursor rotation"),
            [0.0, 1.0, 0.0],
        );
        assert!((cursor_up[0] - 1.0).abs() < 1e-5);
        assert!(cursor_up[1].abs() < 1e-5);
        assert!(cursor_up[2].abs() < 1e-5);
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
        let scene_root = world.add_component_boxed_named(
            "scene_root",
            Box::new(
                TransformComponent::new()
                    .with_position(1.25, 2.5, -3.75)
                    .with_rotation_quat([0.0, 0.38268343, 0.0, 0.9238795]),
            ),
        );
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
        assert_eq!(state.cursor_translation, Some([1.25, 2.5, -3.75]));
        let rotation = state.cursor_rotation.expect("cursor rotation");
        assert!((rotation[0] - 0.0).abs() < 1e-6);
        assert!((rotation[1] - 0.38268343).abs() < 1e-6);
        assert!((rotation[2] - 0.0).abs() < 1e-6);
        assert!((rotation[3] - 0.9238795).abs() < 1e-6);
        assert_eq!(state.cursor_frame, None);
    }
}
