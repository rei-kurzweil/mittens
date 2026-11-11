use crate::engine::{
    Universe,
    ecs::{World, component::EditorUIComponent},
};

#[test]
fn editor_settings_trigger_matrix() {
    for (name, source, expected_fallback) in [
        ("editor_only", "ED {}", true),
        (
            "ui_only",
            "EditorUI { panels([{panel=\"settings\"}]) }",
            false,
        ),
        ("editor_no_panels", "Editor.panels(false) {}", false),
        (
            "editor_and_ui",
            "Editor {} EditorUI { panels([{panel=\"settings\"}]) }",
            false,
        ),
    ] {
        eprintln!("[editor-repro] BEGIN {name}");
        let mut scene = Universe::new(World::default());
        scene
            .load_mms_source_at_path(source, "examples/_editor_trigger_test.mms")
            .unwrap();
        for _ in 0..2 {
            scene.update(0.0, &Default::default());
        }
        let fallback = scene.world.all_components().any(|id| {
            scene.world.component_label(id) == Some("editor_ui_fallback")
                && scene
                    .world
                    .get_component_by_id_as::<EditorUIComponent>(id)
                    .is_some()
        });
        assert_eq!(fallback, expected_fallback, "{name}");
        eprintln!("[editor-repro] END {name}");
    }
}

#[test]
fn sakura_rocket_example_has_no_editor_components() {
    use crate::engine::ecs::component::EditorComponent;
    let mut scene = Universe::new(World::default());
    let source = include_str!("../../examples/sakura-drone-conversion.mms")
        .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
    scene
        .load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")
        .unwrap();
    for _ in 0..2 {
        scene.update(0.0, &Default::default());
    }
    assert!(!scene.world.all_components().any(|id| {
        scene
            .world
            .get_component_by_id_as::<EditorComponent>(id)
            .is_some()
            || scene
                .world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .is_some()
    }));
}
