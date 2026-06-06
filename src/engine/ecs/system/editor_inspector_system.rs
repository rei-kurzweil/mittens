use std::sync::{Arc, Mutex};

use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::editor_context_system::EditorContextState;
use crate::engine::ecs::system::editor_inspector_system_stopgap_mms_adapter::EditorInspectorSystemStopgapMmsAdapter;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};

#[derive(Debug, Default)]
pub struct EditorInspectorSystem {
    stopgap_mms: EditorInspectorSystemStopgapMmsAdapter,
}

impl EditorInspectorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn setup_panels_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        _inspector_panel_pos: (f32, f32, f32),
        editor_context_state: Arc<Mutex<EditorContextState>>,
        asset_system: &crate::engine::ecs::system::AssetSystem,
    ) {
        self.stopgap_mms.setup_panels_for_editor(
            rx,
            world,
            render_assets,
            emit,
            editor_root,
            world_panel_pos,
            _inspector_panel_pos,
            editor_context_state,
            asset_system,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::EditorInspectorSystem;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        EditorComponent, GLTFComponent, OverlayComponent, SerializeComponent, TransformComponent,
    };
    use crate::engine::ecs::system::editor_inspector_system_stopgap_mms_adapter::set_world_panel_scene_path_for_tests;
    use crate::engine::ecs::{EventSignal, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static WORLD_PANEL_SCENE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn find_named_root(world: &World, name: &str) -> crate::engine::ecs::ComponentId {
        world
            .all_components()
            .find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world
                        .component_label(component_id)
                        .is_some_and(|label| label == name)
            })
            .unwrap_or_else(|| panic!("expected root named {name}"))
    }

    fn row_text(
        world: &World,
        root: crate::engine::ecs::ComponentId,
        row_selector: &str,
    ) -> String {
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

    fn count_named_children(
        world: &World,
        root: crate::engine::ecs::ComponentId,
        name: &str,
    ) -> usize {
        world
            .children_of(root)
            .iter()
            .filter(|&&child| {
                world
                    .component_label(child)
                    .is_some_and(|label| label == name)
            })
            .count()
    }

    fn unique_test_scene_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cat_engine_world_panel_{nanos}.mms"))
    }

    #[test]
    fn setup_panels_for_editor_spawns_world_panel_under_root_runtime_ui_transform() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let camera_root =
            world.add_component_boxed_named("camera_root", Box::new(TransformComponent::new()));

        let _ = world.add_child(editor_root, scene_root);

        assert_eq!(world.parent_of(scene_root), Some(editor_root));
        assert!(world.parent_of(camera_root).is_none());

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let panel_mount = world
            .find_component(runtime_ui_root, "#editor_panel_layout_mount")
            .expect("expected panel layout mount under runtime ui root");
        let panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root under runtime ui root");
        assert_eq!(world.parent_of(runtime_ui_root), None);
        assert_eq!(world.parent_of(panel_mount), Some(runtime_ui_root));
        assert!(
            world
                .find_component(runtime_ui_root, "#editor_panel_layout_root")
                .is_some()
        );
        let panel_shared_layout = world
            .parent_of(panel_root)
            .expect("expected shared layout root above world panel root");
        let panel_overlay = world
            .parent_of(panel_shared_layout)
            .expect("expected overlay ancestor above shared layout root");
        assert_eq!(world.parent_of(panel_overlay), Some(panel_mount));
        assert!(
            world
                .get_component_by_id_as::<OverlayComponent>(panel_overlay)
                .is_some()
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#inspector_panel_root")
                .is_some()
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#panel_status_value")
                .is_some()
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#rows_mount")
                .is_some()
        );
        let item0 = world
            .find_component(panel_root, "#item_0")
            .expect("expected item_0 under panel_root");
        println!(
            "item0={:?} type={:?} name={:?}",
            item0,
            world
                .get_component_record(item0)
                .map(|r| r.component_type.clone()),
            world.component_label(item0)
        );
        println!(
            "item0 text descendants={:?}",
            world.find_all_components(item0, "Text")
        );
        assert_eq!(
            row_text(&world, panel_root, "#item_0"),
            "Editor#editor_root"
        );
        assert_eq!(row_text(&world, panel_root, "#item_1"), "scene_root");
    }

    #[test]
    fn setup_panels_for_editor_installs_stopgap_click_status_updates() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);

        assert_eq!(world.parent_of(scene_root), Some(editor_root));

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

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

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

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
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, child_transform);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");

        assert_eq!(
            row_text(&world, runtime_ui_root, "#item_2"),
            "  child_transform"
        );
    }

    #[test]
    fn setup_panels_for_editor_click_updates_inspector_with_selected_component_rows() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, child_transform);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (1.9, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

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

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

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

        let world_panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root");
        let inspector_panel_root = world
            .find_component(runtime_ui_root, "#inspector_panel_root")
            .expect("expected inspector panel root");

        assert!(world.find_component(world_panel_root, "#item_1").is_some());
        assert!(
            world
                .find_component(world_panel_root, "#inspector_item_0")
                .is_none()
        );
        assert!(
            world
                .find_component(inspector_panel_root, "#inspector_item_0")
                .is_some()
        );
        assert!(
            world
                .find_component(inspector_panel_root, "#item_1")
                .is_none()
        );

        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_0"),
            "scene_root"
        );
        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_1"),
            "  child_transform"
        );
    }

    #[test]
    fn setup_panels_for_editor_groups_rows_by_editor_tree() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("alpha", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let other_editor_root =
            world.add_component_boxed_named("beta", Box::new(EditorComponent::new()));
        let other_scene_root =
            world.add_component_boxed_named("other_scene", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(other_editor_root, other_scene_root);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            other_editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

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
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("alpha", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let other_editor_root =
            world.add_component_boxed_named("beta", Box::new(EditorComponent::new()));
        let other_scene_root =
            world.add_component_boxed_named("other_scene", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(other_editor_root, other_scene_root);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );
        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            other_editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        assert_eq!(
            count_named_children(&world, runtime_ui_root, "editor_panel_layout_mount"),
            1
        );
    }

    #[test]
    fn world_panel_save_and_load_buttons_round_trip_filtered_scene_file() {
        let _guard = WORLD_PANEL_SCENE_TEST_LOCK
            .lock()
            .expect("world panel scene test lock poisoned");
        let scene_path = unique_test_scene_path();
        set_world_panel_scene_path_for_tests(Some(scene_path.clone()));

        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let gltf_root = world.add_component_boxed_named(
            "avatar_gltf",
            Box::new(GLTFComponent::new("assets/models/cat/cat.glb")),
        );
        let gltf_serialize = world
            .add_component_boxed_named("avatar_gltf_serialize", Box::new(SerializeComponent::on()));

        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, gltf_root);
        let _ = world.add_child(gltf_root, gltf_serialize);

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
            systems.editor_context.shared_state(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let save_button = world
            .find_component(runtime_ui_root, "#save_button")
            .expect("expected save button");

        systems.rx.push_event(
            save_button,
            EventSignal::Click {
                raycaster: save_button,
                renderable: save_button,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let saved =
            std::fs::read_to_string(&scene_path).expect("expected saved world panel scene file");
        assert!(saved.contains("scene_root"));
        assert!(saved.contains("GLTF.new(\"assets/models/cat/cat.glb\")"));
        assert!(saved.contains("avatar_gltf_serialize"));
        assert!(!saved.contains("editor_runtime_ui_root"));

        world
            .remove_component_subtree(editor_root)
            .expect("expected editor subtree removal to succeed");

        let load_button = world
            .find_component(runtime_ui_root, "#load_button")
            .expect("expected load button");
        systems.rx.push_event(
            load_button,
            EventSignal::Click {
                raycaster: load_button,
                renderable: load_button,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let reloaded_editor_root = world
            .all_components()
            .find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world
                        .get_component_by_id_as::<EditorComponent>(component_id)
                        .is_some()
            })
            .expect("expected reloaded editor root");
        let reloaded_scene_root = world
            .find_component(reloaded_editor_root, "#scene_root")
            .expect("expected scene root after load");
        let reloaded_gltf_root = world
            .find_component(reloaded_scene_root, "#avatar_gltf")
            .expect("expected gltf root after load");
        assert!(
            world
                .find_component(reloaded_gltf_root, "Serialize")
                .is_some()
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#world_panel_root")
                .is_some()
        );

        let panel_status_value = world
            .find_component(runtime_ui_root, "#panel_status_value")
            .expect("expected panel status text after load");
        let status_text = world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(
                panel_status_value,
            )
            .map(|text| text.text.clone())
            .expect("expected panel status text component");
        assert!(status_text.contains("loaded 1 roots"));

        let _ = std::fs::remove_file(&scene_path);
        set_world_panel_scene_path_for_tests(None);
    }
}
