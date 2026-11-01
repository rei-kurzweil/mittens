use super::runner::MeowMeowRunner;
use crate::engine::ecs::component::{EditorPanel, EditorUIComponent};
use crate::engine::ecs::system::SystemWorld;
use crate::engine::ecs::{CommandQueue, ComponentId, SignalEmitter, World};
use crate::engine::graphics::{RenderAssets, VisualWorld};
#[test]
fn bvh_snapshot_controls_can_be_authored_independently_and_start_off() {
    for (toggle, refresh, animation) in [
        (true, true, true),
        (false, true, false),
        (true, false, false),
        (false, false, true),
    ] {
        let source = format!(
            r#"Editor.active() {{ T {{ name="editable_scene" R.cube() }} }}
        T {{ EditorUI {{ panels([{{panel="settings" config={{show_bvh={toggle} show_bvh_refresh={refresh} show_bvh_animation={animation}}}}}]) }} }}"#
        );
        let (mut world, mut systems, mut visuals, mut assets, mut queue) = (
            World::default(),
            SystemWorld::default(),
            VisualWorld::default(),
            RenderAssets::new(),
            CommandQueue::new(),
        );
        let output = MeowMeowRunner::eval_with_world_and_assets(
            &source,
            &mut world,
            &mut systems.rx,
            &mut assets,
            &mut queue,
        );
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        for intent in output.intents {
            queue.push_intent_now(ComponentId::default(), intent);
        }
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        let owner = world.editor_ui_ids().next().unwrap();
        let ui = world
            .get_component_by_id_as::<EditorUIComponent>(owner)
            .unwrap();
        assert!(!ui.bvh_visible);
        assert!(!ui.bvh_animating);
        let config = ui
            .panel_spec(EditorPanel::Settings)
            .unwrap()
            .settings_config()
            .unwrap();
        assert_eq!(
            (
                config.show_bvh,
                config.show_bvh_refresh,
                config.show_bvh_animation
            ),
            (toggle, refresh, animation)
        );
        assert_eq!(
            world
                .find_component(owner, "#editor_settings_bvh_visibility")
                .is_some(),
            toggle
        );
        assert_eq!(
            world
                .find_component(owner, "#editor_settings_bvh_refresh")
                .is_some(),
            refresh
        );
        assert_eq!(
            world
                .find_component(owner, "#editor_settings_bvh_animation")
                .is_some(),
            animation
        );
        let serialized = super::unparser::unparse_component(
            &crate::engine::ecs::component::Component::to_mms_ast(ui, &world),
        );
        assert!(serialized.contains("show_bvh"));
        assert!(serialized.contains("show_bvh_refresh"));
        assert!(serialized.contains("show_bvh_animation"));
        assert!(!serialized.contains("bvh_animating"));
        assert!(!serialized.contains("bvh_revision"));
    }
}
