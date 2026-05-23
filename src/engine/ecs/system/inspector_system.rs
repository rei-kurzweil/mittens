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
            .setup_panels_for_editor(rx, world, emit, editor_root, world_panel_pos);
    }
}

#[cfg(test)]
mod tests {
    use super::InspectorSystem;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        EditorComponent, OverlayComponent, SelectableComponent, TransformComponent,
    };
    use crate::engine::ecs::{EventSignal, SystemWorld, World};
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn setup_panels_for_editor_spawns_world_panel_under_editor_root() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let camera_root = world.add_component_boxed_named("camera_root", Box::new(TransformComponent::new()));

        assert!(world.parent_of(scene_root).is_none());
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

        let panel_mount = world
            .find_component(editor_root, "#editor_world_panel_mount")
            .expect("expected world panel mount under editor root");
        let panel_root = world
            .find_component(editor_root, "#world_panel_root")
            .expect("expected world panel root under editor root");
        assert_eq!(world.parent_of(panel_mount), Some(editor_root));
        let panel_selectable = world
            .parent_of(panel_root)
            .expect("expected selectable-off ancestor above world panel root");
        assert!(world
            .get_component_by_id_as::<SelectableComponent>(panel_selectable)
            .is_some_and(|selectable| !selectable.enabled));
        let panel_overlay = world
            .parent_of(panel_selectable)
            .expect("expected overlay ancestor above selectable-off wrapper");
        assert_eq!(world.parent_of(panel_overlay), Some(panel_mount));
        assert!(world
            .get_component_by_id_as::<OverlayComponent>(panel_overlay)
            .is_some());
        assert!(world.find_component(editor_root, "#panel_status_value").is_some());
        assert!(world.find_component(editor_root, "#rows_mount").is_some());
        assert!(world.find_component(editor_root, "#item_0").is_some());
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

        assert!(world.parent_of(scene_root).is_none());

        inspector.setup_panels_for_editor(
            &mut systems.rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let item_0 = world
            .find_component(editor_root, "#item_0")
            .expect("expected item_0 row under editor root");
        let panel_status_value = world
            .find_component(editor_root, "#panel_status_value")
            .expect("expected panel status text under editor root");

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

        let status_text = world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(
                panel_status_value,
            )
            .map(|text| text.text.clone())
            .expect("expected panel status to remain a text component");
        assert_eq!(status_text, "selected item_0");
    }
}
