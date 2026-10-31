use crate::engine::ecs::system::{
    draggable_system::draggable_owner_for_hit, grabbable_system::grabbable_owner_for_hit,
};
use crate::engine::{
    Universe,
    ecs::{World, component::RenderableComponent},
};
#[test]
fn all_builtin_editor_title_bars_grab_and_drag_the_whole_panel() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"Editor.active() {T {name="editable_scene"}} T {EditorUI}"#,
            "examples/_editor_title_handles.mms",
        )
        .unwrap();
    for _ in 0..3 {
        scene.update(1. / 60., &Default::default());
    }
    for name in [
        "editor_settings_panel_root",
        "paint_panel_root",
        "color_panel_root",
        "grid_panel_root",
        "pose_capture_panel_root",
        "assets_root",
        "world_panel_root",
        "inspector_panel_root",
    ] {
        let panel = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some(name))
            .unwrap_or_else(|| panic!("missing {name}"));
        let title = scene.world.find_component(panel, "#title_bar").unwrap();
        let hits: Vec<_> = scene
            .world
            .owned_subtree(title)
            .into_iter()
            .filter(|&id| {
                scene
                    .world
                    .get_component_by_id_as::<RenderableComponent>(id)
                    .is_some()
            })
            .collect();
        assert!(!hits.is_empty(), "{name} needs rendered title targets");
        for hit in hits {
            assert_eq!(
                grabbable_owner_for_hit(&scene.world, hit),
                Some(panel),
                "grip on {name} must move its complete panel"
            );
            // The minimize button has its own trigger behavior. The title
            // background and text retain their drag target.
            let on_toggle = scene
                .world
                .parents_of(hit)
                .any(|id| scene.world.component_label(id) == Some("accordion_toggle"));
            if !on_toggle {
                assert_eq!(
                    draggable_owner_for_hit(&scene.world, hit),
                    Some(panel),
                    "drag on {name} must move its complete panel"
                );
            }
        }
    }
    let row = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("editor_settings_bvh_visibility"))
        .unwrap();
    assert_eq!(
        grabbable_owner_for_hit(&scene.world, row),
        None,
        "body controls must not become grab handles"
    );
}
