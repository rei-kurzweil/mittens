use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::inspector_system_stopgap_mms_adapter::InspectorSystemStopgapMmsAdapter;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};

#[derive(Debug, Default)]
pub struct InspectorSystem {
    stopgap_mms: InspectorSystemStopgapMmsAdapter,
}

impl InspectorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn setup_panels_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        _inspector_panel_pos: (f32, f32, f32),
    ) {
        self.stopgap_mms
            .setup_panels_for_editor(rx, world, emit, editor_root, world_panel_pos, _inspector_panel_pos);
    }
}

#[cfg(test)]
mod tests {
    use super::InspectorSystem;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{EditorComponent, OverlayComponent, TransformComponent};
    use crate::engine::ecs::{EventSignal, SystemWorld, World};
    use crate::engine::graphics::VisualWorld;

    fn find_named_root(world: &World, name: &str) -> crate::engine::ecs::ComponentId {
        world
            .all_components()
            .find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world.component_label(component_id).is_some_and(|label| label == name)
            })
            .unwrap_or_else(|| panic!("expected root named {name}"))
    }

    fn row_text(world: &World, root: crate::engine::ecs::ComponentId, row_selector: &str) -> String {
        let row = world
            .find_component(root, row_selector)
            .unwrap_or_else(|| panic!("expected row {row_selector}"));
        let text_id = world
            .find_component(row, "Text")
            .unwrap_or_else(|| panic!("expected Text under {row_selector}"));
        world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(text_id)
            .map(|text| text.text.clone())
            .expect("expected text component")
    }

    fn count_named_children(world: &World, root: crate::engine::ecs::ComponentId, name: &str) -> usize {
        world
            .children_of(root)
            .iter()
            .filter(|&&child| world.component_label(child).is_some_and(|label| label == name))
            .count()
    }

    #[test]
    fn setup_panels_for_editor_spawns_world_panel_under_root_runtime_ui_transform() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let camera_root = world.add_component_boxed_named("camera_root", Box::new(TransformComponent::new()));

        let _ = world.add_child(editor_root, scene_root);

        assert_eq!(world.parent_of(scene_root), Some(editor_root));
        assert!(world.parent_of(camera_root).is_none());

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let panel_mount = world
            .find_component(runtime_ui_root, "#editor_panel_layout_mount")
            .expect("expected panel layout mount under runtime ui root");
        let panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root under runtime ui root");
        assert_eq!(world.parent_of(runtime_ui_root), None);
        assert_eq!(world.parent_of(panel_mount), Some(runtime_ui_root));
        assert!(world.find_component(runtime_ui_root, "#editor_panel_layout_root").is_some());
        assert!(world.find_component(runtime_ui_root, "#editor_world_panel_shell").is_some());
        assert!(world.find_component(runtime_ui_root, "#editor_inspector_panel_shell").is_some());
        let panel_overlay = world
            .parent_of(panel_root)
            .expect("expected overlay ancestor above world panel root");
        assert_eq!(world.parent_of(panel_overlay), Some(panel_mount));
        assert!(world
            .get_component_by_id_as::<OverlayComponent>(panel_overlay)
            .is_some());
        assert!(world.find_component(runtime_ui_root, "#inspector_panel_root").is_some());
        assert!(world.find_component(runtime_ui_root, "#panel_status_value").is_some());
        assert!(world.find_component(runtime_ui_root, "#rows_mount").is_some());
        assert!(world.find_component(runtime_ui_root, "#item_0").is_some());
        assert_eq!(row_text(&world, runtime_ui_root, "#item_0"), "Editor#editor_root");
        assert_eq!(row_text(&world, runtime_ui_root, "#item_1"), "scene_root");
    }

    #[test]
    fn setup_panels_for_editor_installs_stopgap_click_status_updates() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);

        assert_eq!(world.parent_of(scene_root), Some(editor_root));

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let item_0 = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");

        systems.rx.push_event(
            item_0,
            EventSignal::Click {
                raycaster: item_0,
                renderable: item_0,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ = systems.process_signals(&mut world, &mut visuals, &mut emit, 100_000);

        let panel_status_value = world
            .find_component(runtime_ui_root, "#panel_status_value")
            .expect("expected panel status text under runtime ui root after rerender");

        let status_text = world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(
                panel_status_value,
            )
            .map(|text| text.text.clone())
            .expect("expected panel status to remain a text component");
        assert_eq!(status_text, "selected scene_root");
    }

    #[test]
    fn setup_panels_for_editor_renders_children_fully_expanded_by_default() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let child_transform = world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, child_transform);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");

        assert_eq!(row_text(&world, runtime_ui_root, "#item_2"), "  child_transform");
    }

    #[test]
    fn setup_panels_for_editor_click_updates_inspector_with_selected_component_mms() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (1.9, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let item_0 = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");

        systems.rx.push_event(
            item_0,
            EventSignal::Click {
                raycaster: item_0,
                renderable: item_0,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ = systems.process_signals(&mut world, &mut visuals, &mut emit, 100_000);

        let panel_status_value = world
            .find_component(runtime_ui_root, "#panel_status_value")
            .expect("expected panel status text under runtime ui root after selection");

        let status_text = world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(
                panel_status_value,
            )
            .map(|text| text.text.clone())
            .expect("expected panel status to remain a text component");
        assert_eq!(status_text, "selected scene_root");

        let inspector_row_text = world
            .find_component(runtime_ui_root, "#inspector_panel_content_root Text")
            .and_then(|text_id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(text_id)
                    .map(|text| text.text.clone())
            })
            .expect("expected inspector panel text after selection");
        assert!(inspector_row_text.contains("name = \"scene_root\""));
    }

    #[test]
    fn setup_panels_for_editor_groups_rows_by_editor_tree() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("alpha", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let other_editor_root = world.add_component_boxed_named("beta", Box::new(EditorComponent::new()));
        let other_scene_root = world.add_component_boxed_named("other_scene", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(other_editor_root, other_scene_root);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );
        systems.process_commands(&mut world, &mut visuals, &mut emit);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            other_editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        assert_eq!(row_text(&world, runtime_ui_root, "#item_0"), "Editor#alpha");
        assert_eq!(row_text(&world, runtime_ui_root, "#item_1"), "scene_root");
        assert_eq!(row_text(&world, runtime_ui_root, "#item_3"), "Editor#beta");
        assert_eq!(row_text(&world, runtime_ui_root, "#item_4"), "other_scene");
    }

    #[test]
    fn setup_panels_for_editor_only_spawns_one_panel_layout_before_command_flush() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("alpha", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let other_editor_root = world.add_component_boxed_named("beta", Box::new(EditorComponent::new()));
        let other_scene_root = world.add_component_boxed_named("other_scene", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(other_editor_root, other_scene_root);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );
        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            other_editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        assert_eq!(count_named_children(&world, runtime_ui_root, "editor_panel_layout_mount"), 1);
    }
}
