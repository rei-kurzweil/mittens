use std::sync::{Arc, Mutex};

use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::editor::context::EditorContextState;
#[cfg(test)]
use crate::engine::ecs::system::editor::inspector_panel::{
    InspectorPanelState, InspectorScrollState, InspectorSubtreeSelection, InspectorWorkspaceEvent,
    InspectorWorkspaceState, reduce_inspector_workspace_state,
};
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
        BoundsComponent, EditorComponent, GLTFComponent, OverlayComponent, RenderableComponent,
        SerializeComponent, TransformComponent,
    };
    use crate::engine::ecs::system::TransformSystem;
    use crate::engine::ecs::system::editor::inspector_panel::{
        InspectorPanelState, InspectorScrollState, InspectorSubtreeSelection,
        InspectorWorkspaceEvent, InspectorWorkspaceState, reduce_inspector_workspace_state,
    };
    use crate::engine::ecs::system::editor_system::select_editor_target;
    use crate::engine::ecs::system::editor_inspector_system_stopgap_mms_adapter::set_world_panel_scene_path_for_tests;
    use crate::engine::ecs::{EventSignal, IntentValue, SignalEmitter, SystemWorld, World};
    use crate::engine::graphics::bounds::Aabb;
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use crate::utils::math::mat_to_quat;
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

    fn text_values_under(
        world: &World,
        root: crate::engine::ecs::ComponentId,
        selector: &str,
    ) -> Vec<String> {
        world
            .find_all_components(root, selector)
            .into_iter()
            .filter_map(|component_id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(
                        component_id,
                    )
                    .map(|text| text.text.clone())
            })
            .collect()
    }

    fn count_children_with_name(
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

    fn flush_runtime_updates(
        systems: &mut SystemWorld,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &RenderAssets,
        emit: &mut CommandQueue,
    ) {
        systems.process_commands(world, visuals, render_assets, emit);
        let _ = systems.process_signals(world, visuals, render_assets, emit, 100_000);
        systems.process_commands(world, visuals, render_assets, emit);
    }

    #[test]
    fn inspector_workspace_reducer_spawns_new_unpinned_panel_when_active_panel_is_pinned() {
        let mut world = World::default();
        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let target_a =
            world.add_component_boxed_named("target_a", Box::new(TransformComponent::new()));
        let target_b =
            world.add_component_boxed_named("target_b", Box::new(TransformComponent::new()));
        let workspace = InspectorWorkspaceState {
            panels: vec![InspectorPanelState {
                panel_id: 1,
                editor_root,
                inspected: Some(target_a),
                pinned: true,
                subtree_selection: InspectorSubtreeSelection::default(),
                scroll_offset: InspectorScrollState::default(),
            }],
            active_panel: Some(1),
            pending_spawn_target: None,
            next_panel_id: 2,
        };

        let reduced = reduce_inspector_workspace_state(
            &workspace,
            &InspectorWorkspaceEvent::SelectionChanged {
                editor_root,
                selected_target: Some(target_b),
            },
        );

        assert_eq!(reduced.panels.len(), 2);
        assert_eq!(reduced.active_panel, Some(2));
        assert_eq!(reduced.panels[0].inspected, Some(target_a));
        assert!(reduced.panels[0].pinned);
        assert_eq!(reduced.panels[1].inspected, Some(target_b));
        assert!(!reduced.panels[1].pinned);
    }

    #[test]
    fn inspector_workspace_reducer_retargets_active_panel_when_unpinned() {
        let mut world = World::default();
        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let target_a =
            world.add_component_boxed_named("target_a", Box::new(TransformComponent::new()));
        let target_b =
            world.add_component_boxed_named("target_b", Box::new(TransformComponent::new()));
        let workspace = InspectorWorkspaceState {
            panels: vec![InspectorPanelState {
                panel_id: 1,
                editor_root,
                inspected: Some(target_a),
                pinned: false,
                subtree_selection: InspectorSubtreeSelection::default(),
                scroll_offset: InspectorScrollState::default(),
            }],
            active_panel: Some(1),
            pending_spawn_target: None,
            next_panel_id: 2,
        };

        let reduced = reduce_inspector_workspace_state(
            &workspace,
            &InspectorWorkspaceEvent::SelectionChanged {
                editor_root,
                selected_target: Some(target_b),
            },
        );

        assert_eq!(reduced.panels.len(), 1);
        assert_eq!(reduced.active_panel, Some(1));
        assert_eq!(reduced.panels[0].inspected, Some(target_b));
    }

    #[test]
    fn setup_panels_for_editor_spawns_world_panel_under_root_runtime_ui_transform() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

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

        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

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
            .expect("expected layout root above world panel root");
        assert!(
            world
                .find_component(panel_shared_layout, "#paint_panel_root")
                .is_some(),
            "expected paint panel under layout root"
        );
        assert!(
            world
                .find_component(panel_shared_layout, "#assets_root")
                .is_some(),
            "expected assets panel under layout root"
        );
        assert!(
            world
                .find_component(panel_shared_layout, "#world_panel_root")
                .is_some(),
            "expected world panel under layout root"
        );
        assert!(
            world
                .find_component(panel_shared_layout, "#inspector_panel_root")
                .is_some(),
            "expected inspector panel under layout root"
        );
        assert_eq!(world.parent_of(panel_root), Some(panel_shared_layout));
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
        systems.selection.install_handlers(&mut systems.rx);

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
        systems.selection.install_handlers(&mut systems.rx);

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
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, child_transform);

        let editor_context_state = systems.editor_context.shared_state();
        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (1.9, 1.6, -1.2),
            editor_context_state.clone(),
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
    fn setup_panels_for_editor_click_rerenders_inspector_and_status_once() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, child_transform);

        let editor_context_state = systems.editor_context.shared_state();
        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (1.9, 1.6, -1.2),
            editor_context_state.clone(),
            &systems.asset_system,
        );

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let world_panel_root = world
            .find_component(runtime_ui_root, "#world_panel_root")
            .expect("expected world panel root");
        let inspector_panel_root = world
            .find_component(runtime_ui_root, "#inspector_panel_root")
            .expect("expected inspector panel root");
        let scene_row = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");

        systems.rx.push_event(
            scene_row,
            EventSignal::Click {
                raycaster: scene_row,
                renderable: scene_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        assert_eq!(
            world
                .find_all_components(world_panel_root, "#panel_status_value")
                .len(),
            1,
            "expected exactly one status label after selection rerender"
        );
        assert_eq!(
            world
                .find_all_components(inspector_panel_root, "#inspector_item_0")
                .len(),
            1,
            "expected exactly one first inspector row after selection rerender"
        );
        assert_eq!(
            world
                .find_all_components(inspector_panel_root, "#inspector_item_1")
                .len(),
            1,
            "expected exactly one second inspector row after selection rerender"
        );
    }

    #[test]
    fn setup_panels_for_editor_pinned_inspector_spawns_second_instance_for_new_selection() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_a =
            world.add_component_boxed_named("scene_a", Box::new(TransformComponent::new()));
        let scene_b =
            world.add_component_boxed_named("scene_b", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_a);
        let _ = world.add_child(editor_root, scene_b);

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
        let scene_a_row = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene_a row under runtime ui root");
        systems.rx.push_event(
            scene_a_row,
            EventSignal::Click {
                raycaster: scene_a_row,
                renderable: scene_a_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        let pin_button = world
            .find_component(runtime_ui_root, "#pin_button")
            .expect("expected pin button on the initial inspector panel");
        systems.rx.push_event(
            pin_button,
            EventSignal::Click {
                raycaster: pin_button,
                renderable: pin_button,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        let scene_b_row = world
            .find_component(runtime_ui_root, "#item_2")
            .expect("expected scene_b row under runtime ui root");
        systems.rx.push_event(
            scene_b_row,
            EventSignal::Click {
                raycaster: scene_b_row,
                renderable: scene_b_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        assert_eq!(
            world
                .find_all_components(runtime_ui_root, "#inspector_panel_root")
                .len(),
            2,
            "expected a second inspector instance after selecting a new target with the first pinned"
        );

        let inspector_roots = world.find_all_components(runtime_ui_root, "#inspector_panel_root");
        assert_eq!(
            inspector_roots.len(),
            2,
            "expected two inspector panel roots"
        );
        let inspector_one = inspector_roots[0];
        let inspector_two = inspector_roots[1];

        assert_eq!(
            row_text(&world, inspector_one, "#inspector_item_0"),
            "scene_a"
        );
        assert_eq!(
            row_text(&world, inspector_two, "#inspector_item_0"),
            "scene_b"
        );

        let detail_one = text_values_under(&world, inspector_one, "Text");
        let detail_two = text_values_under(&world, inspector_two, "Text");
        assert!(
            detail_one.iter().any(|text| text == "scene_a"),
            "expected first pinned inspector detail to stay on scene_a: {detail_one:?}"
        );
        assert!(
            detail_two.iter().any(|text| text == "scene_b"),
            "expected second inspector detail to show scene_b: {detail_two:?}"
        );
    }

    #[test]
    fn setup_panels_for_editor_sidebar_click_updates_detail_without_duplication() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

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
        let scene_row = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");
        systems.rx.push_event(
            scene_row,
            EventSignal::Click {
                raycaster: scene_row,
                renderable: scene_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        let inspector_panel_root = world
            .find_component(runtime_ui_root, "#inspector_panel_root")
            .expect("expected inspector panel root");
        let detail_before = text_values_under(&world, inspector_panel_root, "Text");
        assert_eq!(
            detail_before
                .iter()
                .filter(|text| text.as_str() == "Name")
                .count(),
            1,
            "expected one Name label before sidebar selection: {detail_before:?}"
        );
        assert!(
            detail_before.iter().any(|text| text == "scene_root"),
            "expected initial detail to show scene_root: {detail_before:?}"
        );

        let inspector_child_row = world
            .find_component(inspector_panel_root, "#inspector_item_1")
            .expect("expected child row in inspector sidebar");
        systems.rx.push_event(
            inspector_child_row,
            EventSignal::Click {
                raycaster: inspector_child_row,
                renderable: inspector_child_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        let detail_after = text_values_under(&world, inspector_panel_root, "Text");
        assert_eq!(
            detail_after
                .iter()
                .filter(|text| text.as_str() == "Name")
                .count(),
            1,
            "expected one Name label after sidebar selection: {detail_after:?}"
        );
        assert!(
            detail_after.iter().any(|text| text == "child_transform"),
            "expected detail to update to child_transform: {detail_after:?}"
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
    fn setup_panels_for_editor_hides_editor_helper_subtrees_from_world_and_inspector() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let wrapper = world.add_component_boxed_named(
            "editor_auto_raycastable",
            Box::new(crate::engine::ecs::component::RaycastableComponent::enabled()),
        );
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let gizmo_anchor = world
            .add_component_boxed_named("editor_gizmo_anchor", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, wrapper);
        let _ = world.add_child(wrapper, scene_root);
        let _ = world.add_child(scene_root, child_transform);
        let _ = world.add_child(editor_root, gizmo_anchor);

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
        assert_eq!(
            row_text(&world, runtime_ui_root, "#item_0"),
            "Editor#editor_root"
        );
        assert_eq!(row_text(&world, runtime_ui_root, "#item_1"), "scene_root");
        assert_eq!(
            row_text(&world, runtime_ui_root, "#item_2"),
            "  child_transform"
        );
        assert!(
            world.find_component(runtime_ui_root, "#item_3").is_none(),
            "expected helper wrapper rows to stay hidden"
        );

        let scene_row = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");
        systems.rx.push_event(
            scene_row,
            EventSignal::Click {
                raycaster: scene_row,
                renderable: scene_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_0"),
            "scene_root"
        );
        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_1"),
            "  child_transform"
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#inspector_item_2")
                .is_none(),
            "expected helper subtrees to stay hidden from inspector"
        );
    }

    #[test]
    fn setup_panels_for_editor_parent_changed_does_not_live_refresh_cached_world_rows() {
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
        let sibling_root =
            world.add_component_boxed_named("sibling_root", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(editor_root, sibling_root);

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
        assert_eq!(row_text(&world, runtime_ui_root, "#item_1"), "scene_root");
        assert_eq!(row_text(&world, runtime_ui_root, "#item_2"), "sibling_root");

        emit.push_intent_now(
            editor_root,
            IntentValue::Attach {
                parents: vec![scene_root],
                child: child_transform,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        assert_eq!(row_text(&world, runtime_ui_root, "#item_2"), "sibling_root");
        assert!(
            world.find_component(runtime_ui_root, "#item_3").is_none(),
            "world panel should stay stable until an explicit refresh"
        );

        emit.push_intent_now(
            editor_root,
            IntentValue::Attach {
                parents: vec![sibling_root],
                child: child_transform,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );
        assert_eq!(row_text(&world, runtime_ui_root, "#item_2"), "sibling_root");
        assert!(
            world.find_component(runtime_ui_root, "#item_3").is_none(),
            "world panel should stay stable until an explicit refresh"
        );
    }

    #[test]
    fn setup_panels_for_editor_inspector_skips_runtime_helpers_under_selected_node() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("repro_cube_a", Box::new(TransformComponent::new()));
        let child_transform =
            world.add_component_boxed_named("child_transform", Box::new(TransformComponent::new()));
        let selection_highlight = world
            .add_component_boxed_named("selection_highlight", Box::new(TransformComponent::new()));
        let gizmo_root = world.add_component_boxed_named(
            "editor_transform_gizmo",
            Box::new(TransformComponent::new()),
        );
        let gizmo_visual =
            world.add_component_boxed_named("gizmo_visual", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, child_transform);
        let _ = world.add_child(scene_root, selection_highlight);
        let _ = world.add_child(scene_root, gizmo_root);
        let _ = world.add_child(gizmo_root, gizmo_visual);

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
        let scene_row = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");
        systems.rx.push_event(
            scene_row,
            EventSignal::Click {
                raycaster: scene_row,
                renderable: scene_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );

        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_0"),
            "repro_cube_a"
        );
        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_1"),
            "  child_transform"
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#inspector_item_2")
                .is_none(),
            "expected runtime helper descendants to stay hidden from inspector"
        );
    }

    #[test]
    fn setup_panels_for_editor_inspector_uses_authored_rows_without_runtime_bounds_children() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let renderable_root =
            world.add_component_boxed_named("mesh_root", Box::new(TransformComponent::new()));
        let renderable =
            world.add_component_boxed_named("mesh", Box::new(RenderableComponent::cube()));
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, renderable_root);
        let _ = world.add_child(renderable_root, renderable);

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

        let bounds = world.add_component_boxed(Box::new(BoundsComponent::new(Aabb {
            min: [-0.5, -0.5, -0.5],
            max: [0.5, 0.5, 0.5],
        })));
        let _ = world.add_child(renderable, bounds);

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let scene_row = world
            .find_component(runtime_ui_root, "#item_1")
            .expect("expected scene row under runtime ui root");
        systems.rx.push_event(
            scene_row,
            EventSignal::Click {
                raycaster: scene_row,
                renderable: scene_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_0"),
            "scene_root"
        );
        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_1"),
            "  mesh_root"
        );
        assert_eq!(
            row_text(&world, runtime_ui_root, "#inspector_item_2"),
            "  mesh"
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#inspector_item_3")
                .is_none(),
            "expected runtime bounds children to be hidden from inspector rows"
        );
    }

    #[test]
    fn setup_panels_for_editor_add_grid_click_completes_and_rerenders() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_root);

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
        let add_button = world
            .find_component(runtime_ui_root, "#grid_add_button")
            .expect("expected grid add button");

        systems.rx.push_event(
            add_button,
            EventSignal::Click {
                raycaster: add_button,
                renderable: add_button,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        assert_eq!(row_text(&world, runtime_ui_root, "#grid_item_0"), "grid_1");
        assert_eq!(
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .and_then(|editor| editor.selected),
            None,
            "add-grid should not force scene selection"
        );
        let grid_root = world
            .find_component(editor_root, "#grid_1")
            .expect("expected spawned grid transform under editor root");
        assert!(
            world.find_component(grid_root, "#grid_visual").is_some(),
            "expected a helper visual subtree under the spawned grid"
        );

        let processed =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 10_000);
        assert!(
            processed < 10_000,
            "expected add-grid path to quiesce before signal budget exhaustion"
        );
    }

    #[test]
    fn add_grid_spawns_at_selected_transform_pose() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let target = world.add_component_boxed_named(
            "target",
            Box::new(
                TransformComponent::new()
                    .with_position(1.25, 2.5, -3.75)
                    .with_rotation_quat([0.0, 0.38268343, 0.0, 0.9238795]),
            ),
        );
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, target);

        let editor_context_state = systems.editor_context.shared_state();
        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &render_assets,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (1.9, 1.6, -1.2),
            editor_context_state.clone(),
            &systems.asset_system,
        );

        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        select_editor_target(&mut world, &mut emit, editor_root, target, false);
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );
        {
            let mut editor_context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned");
            editor_context.active_editor = Some(editor_root);
            editor_context.selected_component = Some(target);
            editor_context.cursor_translation = Some([1.25, 2.5, -3.75]);
            editor_context.cursor_rotation = Some([0.0, 0.38268343, 0.0, 0.9238795]);
        }

        let runtime_ui_root = find_named_root(&world, "editor_runtime_ui_root");
        let add_button = world
            .find_component(runtime_ui_root, "#grid_add_button")
            .expect("expected grid add button");

        systems.rx.push_event(
            add_button,
            EventSignal::Click {
                raycaster: add_button,
                renderable: add_button,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        flush_runtime_updates(
            &mut systems,
            &mut world,
            &mut visuals,
            &render_assets,
            &mut emit,
        );

        let grid_root = world
            .find_component(editor_root, "#grid_1")
            .expect("expected spawned grid transform under editor root");
        let grid_transform = world
            .get_component_by_id_as::<TransformComponent>(grid_root)
            .expect("expected grid transform");
        let target_world = TransformSystem::world_model(&world, target).expect("target world pose");
        let expected_rotation = mat_to_quat(target_world);

        assert_eq!(grid_transform.transform.translation, [1.25, 2.5, -3.75]);
        for (actual, expected) in grid_transform
            .transform
            .rotation
            .iter()
            .zip(expected_rotation.iter())
        {
            assert!((actual - expected).abs() < 1.0e-5);
        }
    }
}
