use crate::engine::Universe;
use crate::engine::ecs::component::{RenderableComponent, TextComponent, UnlitComponent};
use crate::engine::ecs::system::material_system::{MaterialSystem, SurfaceSource};
use crate::engine::ecs::{ComponentId, World};

fn named(world: &World, name: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn descendants(world: &World, root: ComponentId) -> Vec<ComponentId> {
    let mut result = Vec::new();
    let mut todo = vec![root];
    while let Some(id) = todo.pop() {
        result.push(id);
        todo.extend_from_slice(world.children_of(id));
    }
    result
}
fn assert_unlit_panel(world: &World, root: ComponentId) {
    assert!(
        world
            .children_of(root)
            .iter()
            .any(|&id| world.get_component_by_id_as::<UnlitComponent>(id).is_some()),
        "panel must own its unlit scope"
    );
    let renderables: Vec<_> = descendants(world, root)
        .into_iter()
        .filter(|&id| {
            world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some()
        })
        .collect();
    assert!(
        renderables.len() > 5,
        "panel should contain generated text and backgrounds"
    );
    for id in renderables {
        assert_eq!(
            MaterialSystem::resolve_surface(world, id),
            Some(SurfaceSource::Unlit),
            "renderable {id:?} did not inherit unlit shading"
        );
    }
}

#[test]
fn panel_backgrounds_and_generated_glyphs_share_non_bloom_ui_material() {
    let source = r#"
        import { info_panel } from "../assets/components/ui/info_panel.mms"
        Editor.active() { T { name = "editable_scene" } }
        T { EditorUI { panels([{ panel = "settings" }, { panel = "world" }]) } }
        T { info_panel({ root_name = "pool_test_panel" width_gu = 24.0 unit_scale = 0.08 title = "Pool"
            content = T { Style { display("block") background_color([0.9, 0.8, 0.6, 1.0]) } Text { "Controls" } }
        }) }
    "#;
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/ui-lighting-test.mms")
        .unwrap();
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    assert_unlit_panel(&scene.world, named(&scene.world, "pool_test_panel"));
    assert_unlit_panel(
        &scene.world,
        named(&scene.world, "editor_settings_panel_root"),
    );
    assert_unlit_panel(&scene.world, named(&scene.world, "world_panel_root"));
}

#[test]
fn text_unlit_marker_reaches_generated_glyphs() {
    let mut world = World::default();
    let text = world.add_component(TextComponent::new("ABC"));
    let marker = world.add_component(UnlitComponent);
    world.add_child(text, marker).unwrap();
    let glyphs = crate::engine::ecs::system::TextSystem::default().register_text(
        &mut world,
        &mut Default::default(),
        text,
    );
    assert_eq!(glyphs.len(), 3);
    for glyph in glyphs {
        let renderable = world
            .children_of(glyph.transform)
            .iter()
            .copied()
            .find(|&id| {
                world
                    .get_component_by_id_as::<RenderableComponent>(id)
                    .is_some()
            })
            .unwrap();
        assert_eq!(
            MaterialSystem::resolve_surface(&world, renderable),
            Some(SurfaceSource::Unlit)
        );
    }
}

#[test]
fn sakura_pool_scene_panels_are_unlit() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            include_str!("../../examples/sakura-drone-conversion-desktop.mms"),
            "examples/sakura-drone-conversion-desktop.mms",
        )
        .unwrap();
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    for name in [
        "sakura_scene_controls",
        "sakura_graphics_settings",
        "sakura_xr_settings",
    ] {
        assert_unlit_panel(&scene.world, named(&scene.world, name));
    }
}
