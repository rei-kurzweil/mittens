use std::sync::{Arc, Mutex};

use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::editor_context_system::EditorContextState;
use crate::engine::ecs::system::editor_inspector_system_stopgap_mms_adapter::EditorInspectorSystemStopgapMmsAdapter;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};

pub(crate) type InspectorPanelId = u64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorSubtreeSelection {
    pub(crate) focused_row: Option<ComponentId>,
    pub(crate) expanded: Vec<ComponentId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorScrollState {
    pub(crate) row_offset: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorPanelState {
    pub(crate) panel_id: InspectorPanelId,
    pub(crate) editor_root: ComponentId,
    pub(crate) inspected: Option<ComponentId>,
    pub(crate) pinned: bool,
    pub(crate) subtree_selection: InspectorSubtreeSelection,
    pub(crate) scroll_offset: InspectorScrollState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorWorkspaceState {
    pub(crate) panels: Vec<InspectorPanelState>,
    pub(crate) active_panel: Option<InspectorPanelId>,
    pub(crate) pending_spawn_target: Option<ComponentId>,
    pub(crate) next_panel_id: InspectorPanelId,
}

impl InspectorWorkspaceState {
    pub(crate) fn next_panel_id(&mut self) -> InspectorPanelId {
        let next = self.next_panel_id.max(1);
        self.next_panel_id = next + 1;
        next
    }

    pub(crate) fn active_panel_index(&self) -> Option<usize> {
        let active_panel = self.active_panel?;
        self.panels
            .iter()
            .position(|panel| panel.panel_id == active_panel)
    }

    pub(crate) fn ensure_default_panel(
        &mut self,
        editor_root: ComponentId,
        inspected: Option<ComponentId>,
    ) -> InspectorPanelId {
        if let Some(panel) = self.panels.first() {
            return panel.panel_id;
        }

        let panel_id = self.next_panel_id();
        self.panels.push(InspectorPanelState {
            panel_id,
            editor_root,
            inspected,
            pinned: false,
            subtree_selection: InspectorSubtreeSelection::default(),
            scroll_offset: InspectorScrollState::default(),
        });
        self.active_panel = Some(panel_id);
        panel_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InspectorWorkspaceEvent {
    SelectionChanged {
        editor_root: ComponentId,
        selected_target: Option<ComponentId>,
    },
    PanelFocused {
        panel_id: InspectorPanelId,
    },
    PanelPinToggled {
        panel_id: InspectorPanelId,
    },
}

pub(crate) fn clear_missing_inspector_targets(
    workspace: &mut InspectorWorkspaceState,
    component_exists: impl Fn(ComponentId) -> bool,
) {
    for panel in &mut workspace.panels {
        if panel
            .inspected
            .is_some_and(|component_id| !component_exists(component_id))
        {
            panel.inspected = None;
        }
    }
}

pub(crate) fn reduce_inspector_workspace_state(
    old: &InspectorWorkspaceState,
    event: &InspectorWorkspaceEvent,
) -> InspectorWorkspaceState {
    let mut new = old.clone();

    match event {
        InspectorWorkspaceEvent::SelectionChanged {
            editor_root,
            selected_target,
        } => {
            if new.panels.is_empty() {
                new.ensure_default_panel(*editor_root, *selected_target);
                return new;
            }

            let active_index = new.active_panel_index().unwrap_or(0);
            let active_panel = &new.panels[active_index];
            let should_spawn = active_panel.pinned
                && selected_target.is_some()
                && active_panel.inspected != *selected_target;

            if should_spawn {
                let panel_id = new.next_panel_id();
                new.panels.insert(
                    active_index + 1,
                    InspectorPanelState {
                        panel_id,
                        editor_root: *editor_root,
                        inspected: *selected_target,
                        pinned: false,
                        subtree_selection: InspectorSubtreeSelection::default(),
                        scroll_offset: InspectorScrollState::default(),
                    },
                );
                new.active_panel = Some(panel_id);
                new.pending_spawn_target = None;
                return new;
            }

            let active_panel = &mut new.panels[active_index];
            active_panel.editor_root = *editor_root;
            active_panel.inspected = *selected_target;
            new.active_panel = Some(active_panel.panel_id);
            new.pending_spawn_target = None;
        }
        InspectorWorkspaceEvent::PanelFocused { panel_id } => {
            new.active_panel = Some(*panel_id);
        }
        InspectorWorkspaceEvent::PanelPinToggled { panel_id } => {
            new.active_panel = Some(*panel_id);
            if let Some(panel) = new
                .panels
                .iter_mut()
                .find(|panel| panel.panel_id == *panel_id)
            {
                panel.pinned = !panel.pinned;
            }
        }
    }

    new
}

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
    use super::{
        EditorInspectorSystem, InspectorPanelState, InspectorScrollState,
        InspectorSubtreeSelection, InspectorWorkspaceEvent, InspectorWorkspaceState,
        reduce_inspector_workspace_state,
    };
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        BoundsComponent, EditorComponent, GLTFComponent, OverlayComponent, RenderableComponent,
        SerializeComponent, TransformComponent,
    };
    use crate::engine::ecs::system::editor_inspector_system_stopgap_mms_adapter::set_world_panel_scene_path_for_tests;
    use crate::engine::ecs::{EventSignal, IntentValue, SignalEmitter, SystemWorld, World};
    use crate::engine::graphics::bounds::Aabb;
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
            .expect("expected layout root above world panel root");
        assert!(
            world.find_component(panel_shared_layout, "#paint_panel_root").is_some(),
            "expected paint panel under layout root"
        );
        assert!(
            world.find_component(panel_shared_layout, "#assets_root").is_some(),
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
            "    mesh"
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#inspector_item_3")
                .is_none(),
            "expected runtime Bounds child to stay hidden from inspector"
        );
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
    fn setup_panels_for_editor_respawns_panel_layout_when_runtime_panel_subtree_was_removed() {
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
        let _ = world.add_child(editor_root, scene_root);

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
            .expect("expected initial panel layout mount");
        world
            .remove_component_subtree(panel_mount)
            .expect("expected initial panel layout mount subtree removal to succeed");

        assert!(
            world
                .find_component(runtime_ui_root, "#editor_panel_layout_mount")
                .is_none(),
            "expected panel layout mount subtree to be removed before respawn"
        );

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

        assert_eq!(
            count_named_children(&world, runtime_ui_root, "editor_panel_layout_mount"),
            1,
            "expected panel layout mount to respawn exactly once after subtree removal"
        );
        assert!(
            world
                .find_component(runtime_ui_root, "#editor_panel_layout_root")
                .is_some(),
            "expected panel layout root to respawn with the panel layout"
        );
        let layout_root = world
            .find_component(runtime_ui_root, "#editor_panel_layout_root")
            .expect("panel layout root");
        assert!(
            count_children_with_name(&world, layout_root, "inspector_panel_root") > 0,
            "expected inspector panel instances to attach under the layout root"
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

    #[test]
    fn world_panel_load_ignores_editor_panel_roots_from_legacy_scene_files() {
        let _guard = WORLD_PANEL_SCENE_TEST_LOCK
            .lock()
            .expect("world panel scene test lock poisoned");
        let scene_path = unique_test_scene_path();
        set_world_panel_scene_path_for_tests(Some(scene_path.clone()));

        std::fs::write(
            &scene_path,
            r#"
T {
    name = "inspector_panel_root"
    Style {}
    T {
        name = "title_bar"
        T {
            Text { "Inspector" }
        }
    }
}

Editor.panels(true) {
    name = "editor_root"
    T {
        name = "scene_root"
    }
}
"#,
        )
        .expect("expected test scene file write to succeed");

        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::new();
        let mut inspector = EditorInspectorSystem::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));

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

        let loaded_editor_root = world
            .all_components()
            .find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world.component_label(component_id) == Some("editor_root")
                    && component_id != editor_root
            })
            .expect("expected authored editor root to load");
        assert!(
            world
                .find_component(loaded_editor_root, "#scene_root")
                .is_some(),
            "expected authored scene root to load"
        );
        assert_eq!(
            world
                .find_all_components(runtime_ui_root, "#inspector_panel_root")
                .len(),
            1,
            "expected only the runtime inspector panel root to remain after load"
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
